import { vi } from "vitest";
import type { Cursor, EntryScope, EntryView, Feed, Folder } from "../api";

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
        return this.feeds;
      case "list_folders":
        return this.folders;
      case "entry_counts":
        return {
          all_entries: this.entries.length,
          starred_entries: this.entries.filter((e) => e.is_starred).length,
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
      case "set_starred": {
        const entry = this.entries.find((e) => e.id === args.entryId);
        if (entry) entry.is_starred = args.isStarred === true;
        return null;
      }
      case "refresh_all":
        return [];
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

  private filtered(args: Args): StoredEntry[] {
    const view = args.view as EntryView;
    const scope = args.scope as EntryScope;
    return this.entries.filter((e) => {
      if (scope.kind === "feed" && e.feed_id !== scope.id) return false;
      if (scope.kind === "starred" && !e.is_starred) return false;
      if (scope.kind === "folder") return false; // no folders seeded in tests
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
