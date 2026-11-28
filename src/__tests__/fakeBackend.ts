import { vi } from "vitest";
import type {
  Cursor,
  EntryScope,
  EntryView,
  Feed,
  FeedRefresh,
  Folder,
  RefreshSettings,
  SyncConfig,
  SyncStatus,
} from "../api";

/** A stored entry as the backend sees it: rows carry their ordering position,
 *  full entries carry their body. */
export interface StoredEntry {
  id: number;
  feed_id: number;
  title: string;
  link: string | null;
  content_html: string;
  excerpt: string;
  image_url: string | null;
  published_at: string | null;
  sort_key: string;
  is_read: boolean;
  is_starred: boolean;
}

/** Lexicographically ordered, increasing with id — the same shape the Rust
 *  side stores (`COALESCE(published_at, created_at)`). */
function stamp(i: number): string {
  const h = String(Math.floor(i / 60)).padStart(2, "0");
  const m = String(i % 60).padStart(2, "0");
  return `2026-01-01T${h}:${m}:00+00:00`;
}

type Args = Record<string, unknown>;

/**
 * Stand-in for the Tauri IPC bridge. Implements the same paging contract as
 * `db.rs` — sort key descending, id descending, keyset cursor, `limit + 1`
 * rows so callers can derive `hasMore` — so tests exercise the real
 * frontend logic rather than a simplified version of it.
 */
export class FakeBackend {
  readonly entries: StoredEntry[] = [];
  readonly feeds: Feed[] = [];
  readonly folders: Folder[] = [];
  readonly calls: { cmd: string; args: Args }[] = [];

  /** The saved WebDAV connection; null means it was never configured. */
  syncConfig: SyncConfig | null = null;

  /** The saved refresh schedule; the default matches `db.rs`. */
  refreshSettings: RefreshSettings = { enabled: false, interval_minutes: 30 };

  /** Per-feed refresh answers, by feed id. A feed without an entry is
   *  refreshed successfully with no new entries. Cleared by `resetRefreshes`. */
  refreshAnswers = new Map<number, Partial<FeedRefresh> | "throw">();

  /** What every refresh of one feed reports, unless overridden above. */
  defaultRefresh: Partial<FeedRefresh> = {};

  /** What the next sync reports. */
  syncOutcome: SyncStatus = {
    ran_at: new Date(0).toISOString(),
    outcome: "nothing_changed",
    detail: "no changes on either side",
    applied: false,
  };

  private gates: { cmd: string; promise: Promise<void> }[] = [];

  constructor(options: { entries?: number; feeds?: number } = {}) {
    const feedCount = options.feeds ?? 1;
    for (let f = 1; f <= feedCount; f++) {
      this.feeds.push({
        id: f,
        url: `https://f${f}.example/feed`,
        title: `Feed ${f}`,
        site_url: null,
        last_fetched_at: null,
        last_error: null,
        folder_id: null,
        unread_count: 0,
      });
    }
    const count = options.entries ?? 120;
    for (let i = 1; i <= count; i++) {
      this.entries.push({
        id: i,
        feed_id: ((i - 1) % feedCount) + 1,
        title: `Entry ${i}`,
        link: `https://example.test/${i}`,
        content_html: `<p>Body ${i}</p>`,
        excerpt: `Body ${i}`,
        // Every other entry carries a cover, so tests see both row shapes.
        image_url: i % 2 === 0 ? `https://img.example/${i}.jpg` : null,
        published_at: stamp(i),
        sort_key: stamp(i),
        // Every third entry starts read: 120 seeded -> 40 read, 80 unread.
        is_read: i % 3 === 0,
        is_starred: false,
      });
    }
  }

  /** Hold the next call of `cmd` open until `release()` is called. Used to
   *  put a batch in flight while the reader changes selection. */
  holdNext(cmd: string): { release: () => void } {
    let release!: () => void;
    const promise = new Promise<void>((resolve) => {
      release = resolve;
    });
    this.gates.push({ cmd, promise });
    return { release };
  }

  readonly invoke = async (cmd: string, args: Args = {}): Promise<unknown> => {
    this.calls.push({ cmd, args });
    const idx = this.gates.findIndex((g) => g.cmd === cmd);
    if (idx >= 0) await this.gates.splice(idx, 1)[0].promise;
    return this.dispatch(cmd, args);
  };

  /** How often `cmd` was called, optionally with a matching argument. */
  countCalls(cmd: string, match?: (args: Args) => boolean): number {
    return this.calls.filter(
      (c) => c.cmd === cmd && (!match || match(c.args)),
    ).length;
  }

  private dispatch(cmd: string, args: Args): unknown {
    switch (cmd) {
      case "list_feeds":
        return this.feeds.map((f) => ({
          ...f,
          unread_count: this.unreadFor(f.id),
        }));
      case "list_folders":
        return this.folders.map((folder) => {
          const feeds = this.feeds
            .filter((f) => f.folder_id === folder.id)
            .map((f) => ({ ...f, unread_count: this.unreadFor(f.id) }));
          return {
            ...folder,
            feeds,
            unread_count: feeds.reduce((n, f) => n + f.unread_count, 0),
          };
        });
      case "entry_counts":
        return {
          all_entries: this.entries.length,
          starred_entries: this.entries.filter((e) => e.is_starred).length,
          unread_entries: this.entries.filter((e) => !e.is_read).length,
        };
      case "list_entries":
        return this.listEntries(args);
      case "count_entries":
        return this.filtered(args).length;
      case "get_entry": {
        const found = this.entries.find((e) => e.id === args.entryId);
        return found ? { ...found } : null;
      }
      case "set_read": {
        const entry = this.entries.find((e) => e.id === args.entryId);
        if (entry) entry.is_read = args.isRead === true;
        return null;
      }
      case "mark_read_many": {
        // Mirrors `db::mark_read_many`: only unread rows count as changed.
        const ids = (args.ids ?? []) as number[];
        let changed = 0;
        for (const entry of this.entries) {
          if (ids.includes(entry.id) && !entry.is_read) {
            entry.is_read = true;
            changed += 1;
          }
        }
        return changed;
      }
      case "mark_scope_read": {
        // Mirrors `db::mark_scope_read`: the unread view of the scope is
        // exactly the set that changes, loaded rows or not.
        const scope = args.scope as EntryScope;
        const targets = this.filtered({ view: "unread", scope });
        for (const entry of targets) entry.is_read = true;
        return targets.length;
      }
      case "set_starred": {
        const entry = this.entries.find((e) => e.id === args.entryId);
        if (entry) entry.is_starred = args.isStarred === true;
        return null;
      }
      case "refresh_feed": {
        const feedId = args.feedId as number;
        const answer = this.refreshAnswers.get(feedId);
        if (answer === "throw") throw new Error(`refresh of feed ${feedId} failed`);
        const merged: FeedRefresh = {
          feed_id: feedId,
          error: null,
          new_entries: 0,
          http_status: null,
          ...this.defaultRefresh,
          ...(answer ?? {}),
        };
        this.feeds.forEach((feed) => {
          if (feed.id !== feedId) return;
          feed.last_fetched_at = new Date().toISOString();
          feed.last_error = merged.error;
        });
        return merged;
      }
      case "starred_feed_ids":
        return [...new Set(
          this.entries.filter((e) => e.is_starred).map((e) => e.feed_id),
        )];
      case "load_refresh_settings":
        return { ...this.refreshSettings };
      case "save_refresh_settings": {
        const settings = args.settings as RefreshSettings;
        this.refreshSettings = { ...settings };
        return null;
      }
      case "load_sync_config":
        return this.syncConfig;
      case "sync_now":
        return this.syncOutcome;
      case "sync_status":
        return this.syncConfig ? this.syncOutcome : null;
      case "remove_feed": {
        const feedId = args.feedId as number;
        const keep = this.entries.filter((e) => e.feed_id !== feedId);
        this.entries.splice(0, this.entries.length, ...keep);
        return null;
      }
      default:
        return null;
    }
  }

  private listEntries(args: Args): StoredEntry[] {
    const view = args.view as EntryView;
    const scope = args.scope as EntryScope;
    const after = (args.after ?? null) as Cursor | null;
    const limit = args.limit as number;

    const ordered = this.filtered({ view, scope }).sort(
      (a, b) =>
        b.sort_key.localeCompare(a.sort_key) || b.id - a.id,
    );
    const tail = after
      ? ordered.filter(
          (e) =>
            e.sort_key < after.sort ||
            (e.sort_key === after.sort && e.id < after.id),
        )
      : ordered;
    // One row beyond the page, exactly like `db::list_entries`.
    return tail.slice(0, limit + 1).map((e) => ({ ...e }));
  }

  /** Unread entries of one feed, the figure `db::list_feeds` computes in SQL. */
  unreadFor(feedId: number): number {
    return this.entries.filter((e) => e.feed_id === feedId && !e.is_read).length;
  }

  private filtered(args: Args): StoredEntry[] {
    const view = args.view as EntryView;
    const scope = args.scope as EntryScope;
    return this.entries.filter((e) => {
      if (scope.kind === "feed" && e.feed_id !== scope.id) return false;
      if (scope.kind === "starred" && !e.is_starred) return false;
      if (scope.kind === "folder") {
        const inFolder = this.feeds.some(
          (f) => f.id === e.feed_id && f.folder_id === scope.id,
        );
        if (!inFolder) return false;
      }
      if (view === "unread" && e.is_read) return false;
      return true;
    });
  }
}

/** jsdom reports a zero rect for everything, which makes the sentinel look
 *  permanently on screen and would chain batches to the end of the list.
 *  Report a realistic list box instead, with the sentinel either inside it
 *  (so loading keeps chaining) or below it (so one batch is one batch). */
export function stubScrollGeometry(sentinelVisible: boolean): void {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
    function (this: Element): DOMRect {
      const rect = (top: number, bottom: number): DOMRect => ({
        top,
        bottom,
        left: 0,
        right: 100,
        width: 100,
        height: bottom - top,
        x: 0,
        y: top,
        toJSON: () => ({}),
      });
      if (this.classList?.contains("sentinel")) {
        return sentinelVisible ? rect(10, 20) : rect(1000, 1010);
      }
      return rect(0, 500);
    },
  );
}

/**
 * Geometry for a list that actually grows: the sentinel sits immediately after
 * the last rendered row, and revealing the last row scrolls the box to its
 * bottom. `stubScrollGeometry` cannot express that — it pins the sentinel to a
 * fixed rect, so the sentinel never leaves the box as rows are appended, and a
 * bug that asks for another batch before the appended rows are laid out stays
 * invisible.
 */
export function stubGrowingListGeometry(
  options: { rowHeight?: number; boxHeight?: number } = {},
): void {
  const rowHeight = options.rowHeight ?? 120;
  const boxHeight = options.boxHeight ?? 600;
  let scrollTop = 0;

  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(
    function (this: Element): DOMRect {
      const rect = (top: number, bottom: number): DOMRect =>
        ({
          top,
          bottom,
          left: 0,
          right: 100,
          width: 100,
          height: bottom - top,
          x: 0,
          y: top,
          toJSON: () => ({}),
        }) as DOMRect;
      if (this.classList?.contains("sentinel")) {
        const rows = document.querySelectorAll(".entry").length;
        const top = rows * rowHeight - scrollTop;
        return rect(top, top + 20);
      }
      return rect(0, boxHeight);
    },
  );

  Object.defineProperty(Element.prototype, "scrollIntoView", {
    value: () => {
      const rows = document.querySelectorAll(".entry").length;
      scrollTop = Math.max(0, rows * rowHeight - boxHeight);
    },
    writable: true,
    configurable: true,
  });
}

/** An `IntersectionObserver` that records itself so tests can fire it. */
export class FakeIntersectionObserver {
  static instances: FakeIntersectionObserver[] = [];

  target: Element | null = null;

  constructor(
    readonly callback: IntersectionObserverCallback,
    readonly options?: IntersectionObserverInit,
  ) {
    FakeIntersectionObserver.instances.push(this);
  }

  observe(target: Element): void {
    this.target = target;
  }

  unobserve(): void {}

  disconnect(): void {}

  /** Simulate the sentinel entering (or leaving) the scroll port. */
  trigger(isIntersecting: boolean): void {
    this.callback(
      [{ isIntersecting, target: this.target } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver,
    );
  }

  static reset(): void {
    FakeIntersectionObserver.instances = [];
  }

  static last(): FakeIntersectionObserver {
    const all = FakeIntersectionObserver.instances;
    const instance = all[all.length - 1];
    if (!instance) throw new Error("no IntersectionObserver was created");
    return instance;
  }
}

/** Let every queued microtask and macrotask settle. */
export async function flush(times = 8): Promise<void> {
  for (let i = 0; i < times; i++) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

/**
 * A viewport the test controls, standing in for `window.matchMedia`.
 *
 * The application asks the viewport three separate questions — too narrow for
 * three columns, coarse and too short for three usable columns, and coarse on
 * its own — plus whether a pointer can hover. Answering them from one value
 * cannot express a tablet in landscape, so each answer gets its own list.
 */
export class FakeMediaQueryList {
  matches: boolean;
  private listeners = new Set<() => void>();

  constructor(matches: boolean) {
    this.matches = matches;
  }

  addEventListener(_type: "change", listener: () => void) {
    this.listeners.add(listener);
  }

  removeEventListener(_type: "change", listener: () => void) {
    this.listeners.delete(listener);
  }

  resize(matches: boolean) {
    this.matches = matches;
    for (const listener of this.listeners) listener();
  }

  get listenerCount(): number {
    return this.listeners.size;
  }
}

export interface ViewportAnswers {
  /** `(max-width: 800px)` — too narrow for three columns. */
  narrow?: boolean;
  /** `(max-width: 1023px) and (pointer: coarse)` — a finger, no room. */
  coarseNarrow?: boolean;
  /** `(pointer: coarse)`. */
  coarse?: boolean;
  /** `(hover: hover) and (pointer: fine)`. */
  hover?: boolean;
}

export interface InstalledViewport {
  narrow: FakeMediaQueryList;
  coarseNarrow: FakeMediaQueryList;
  coarse: FakeMediaQueryList;
  hover: FakeMediaQueryList;
  /** Answer every query afresh, as a viewport change would. */
  set(answers: ViewportAnswers): void;
}

/** Install `matchMedia` for a test. Call from `beforeEach`, after stubbing. */
export function installViewport(answers: ViewportAnswers = {}): InstalledViewport {
  const lists = {
    narrow: new FakeMediaQueryList(answers.narrow ?? false),
    coarseNarrow: new FakeMediaQueryList(answers.coarseNarrow ?? false),
    coarse: new FakeMediaQueryList(answers.coarse ?? false),
    hover: new FakeMediaQueryList(answers.hover ?? false),
  };

  vi.stubGlobal("matchMedia", (query: string) => {
    if (query.includes("hover")) return lists.hover;
    if (query.includes("pointer: coarse") && query.includes("max-width")) {
      return lists.coarseNarrow;
    }
    if (query.includes("pointer: coarse")) return lists.coarse;
    return lists.narrow;
  });

  return {
    ...lists,
    set(next: ViewportAnswers) {
      if (next.narrow !== undefined) lists.narrow.resize(next.narrow);
      if (next.coarseNarrow !== undefined) lists.coarseNarrow.resize(next.coarseNarrow);
      if (next.coarse !== undefined) lists.coarse.resize(next.coarse);
      if (next.hover !== undefined) lists.hover.resize(next.hover);
    },
  };
}
