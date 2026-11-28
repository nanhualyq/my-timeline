import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App.vue";
import EntryList from "../components/EntryList.vue";
import ReadingPane from "../components/ReadingPane.vue";
import Sidebar from "../components/Sidebar.vue";
import type { EntryListItem } from "../api";
import {
  FakeBackend,
  FakeIntersectionObserver,
  flush,
  stubScrollGeometry,
} from "./fakeBackend";

/** The IPC bridge the mocked `@tauri-apps/api/core` dispatches into. */
const harness = vi.hoisted(() => ({
  backend: {
    invoke: (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> =>
      Promise.resolve(null),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) =>
    harness.backend.invoke(cmd, args),
}));

const dialog = vi.hoisted(() => ({
  message: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: dialog.message,
  confirm: async () => true,
  open: async () => null,
  save: async () => null,
}));

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => undefined }));

let backend: FakeBackend;

async function mountApp(
  options: {
    entries?: number;
    feeds?: number;
    view?: "all" | "unread";
    /** A saved WebDAV connection, as the settings dialog would have left it. */
    sync?: {
      url?: string;
      outcome?: "synced" | "created" | "nothing_changed" | "failed";
      /** Whether that sync changed the library; defaults to "it did not". */
      applied?: boolean;
    };
  } = {},
): Promise<VueWrapper> {
  backend = new FakeBackend(options);
  if (options.sync) {
    backend.syncConfig = {
      url: options.sync.url ?? "https://dav.example/dav",
      path: "",
      username: "me",
      password: "secret",
      accept_invalid_certs: false,
    };
    backend.syncOutcome = {
      ran_at: new Date(0).toISOString(),
      outcome: options.sync.outcome ?? "nothing_changed",
      detail: "1 feed(s) added",
      applied: options.sync.applied ?? false,
    };
  }
  harness.backend = backend;
  const wrapper = mount(App);
  await flush();
  if (options.view === "all") {
    // Every list opens on the unread filter; specs that assert paging
    // arithmetic over every stored entry ask for the All tab explicitly.
    list(wrapper).vm.$emit("setView", "all");
    await flush();
  }
  return wrapper;
}

function list(wrapper: VueWrapper) {
  return wrapper.findComponent(EntryList);
}

function pane(wrapper: VueWrapper) {
  return wrapper.findComponent(ReadingPane);
}

function rows(wrapper: VueWrapper): EntryListItem[] {
  return list(wrapper).props("entries") as EntryListItem[];
}

function rowById(wrapper: VueWrapper, id: number): EntryListItem {
  const found = rows(wrapper).find((r) => r.id === id);
  if (!found) throw new Error(`entry ${id} is not in the loaded rows`);
  return found;
}

function listCalls() {
  return backend.calls.filter((c) => c.cmd === "list_entries");
}

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  // The sentinel sits below the fold: one `loadMore` is one batch.
  stubScrollGeometry(false);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("batch loading", () => {
  it("loads one bounded batch and reports progress", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });

    expect(rows(wrapper)).toHaveLength(50);
    expect(list(wrapper).props("hasMore")).toBe(true);
    expect(list(wrapper).props("viewedCount")).toBe(50);
    expect(list(wrapper).props("total")).toBe(120);

    const first = listCalls()[0];
    expect(first.args.limit, "never asks for more than one batch").toBe(50);
    expect(first.args.after).toBeNull();
  });

  it("appends the next batch from the last loaded row with no gaps", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });
    const first = rows(wrapper);

    list(wrapper).vm.$emit("loadMore");
    await flush();

    const loaded = rows(wrapper);
    expect(loaded).toHaveLength(100);
    // Newest first, descending by one: no repeats and nothing skipped.
    expect(loaded.map((r) => r.id)).toEqual(
      Array.from({ length: 100 }, (_, i) => 120 - i),
    );

    const calls = listCalls();
    const second = calls[calls.length - 1];
    expect(second.args.after).toEqual({
      sort: first[49].sort_key,
      id: first[49].id,
    });
  });

  it("does not ask for another batch once the list is exhausted", async () => {
    const wrapper = await mountApp({ entries: 60, feeds: 1, view: "all" });

    list(wrapper).vm.$emit("loadMore");
    await flush();
    expect(rows(wrapper)).toHaveLength(60);
    expect(list(wrapper).props("hasMore")).toBe(false);

    const before = listCalls().length;
    list(wrapper).vm.$emit("loadMore");
    await flush();
    expect(listCalls()).toHaveLength(before);
  });

  it("treats an empty batch as the end without dropping what is loaded", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });
    expect(rows(wrapper)).toHaveLength(50);

    // Everything the cursor points at is deleted between batches.
    backend.entries.splice(0, backend.entries.length);

    list(wrapper).vm.$emit("loadMore");
    await flush();

    expect(list(wrapper).props("hasMore")).toBe(false);
    expect(rows(wrapper), "the loaded rows stay on screen").toHaveLength(50);

    const before = listCalls().length;
    list(wrapper).vm.$emit("loadMore");
    await flush();
    expect(listCalls(), "it does not keep asking").toHaveLength(before);
  });
});

describe("selection and refresh", () => {
  it("discards an in-flight batch when the selection changes", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2, view: "all" });

    // Put a batch of the current (global) selection in flight...
    const gate = backend.holdNext("list_entries");
    list(wrapper).vm.$emit("loadMore");
    await flush();

    // ...then change scope before it lands.
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();
    gate.release();
    await flush();

    const loaded = rows(wrapper);
    expect(loaded.length).toBeGreaterThan(0);
    expect(
      loaded.every((r) => r.feed_id === 2),
      `stale rows leaked in: ${loaded.map((r) => r.feed_id).join(",")}`,
    ).toBe(true);
    expect(list(wrapper).props("hasMore"), "feed 2 still has more").toBe(true);
  });

  it("reports a folder refresh in the panel instead of a dialog", async () => {
    const wrapper = await mountApp({ entries: 12, feeds: 2, view: "all" });
    backend.folders.push({ id: 1, name: "Tech", unread_count: 0, feeds: [] });
    backend.feeds[1].folder_id = 1;
    // Pick the assignment up the way the sidebar does after an edit.
    wrapper.findComponent(Sidebar).vm.$emit("changed");
    await flush();

    wrapper.findComponent(Sidebar).vm.$emit("refreshFolder", 1);
    await flush();

    // Only the folder's feed was fetched, and the outcome is a notice rather
    // than a modal the reader has to dismiss.
    expect(backend.countCalls("refresh_feed")).toBe(1);
    expect(wrapper.find(".refresh-panel").exists()).toBe(true);
    expect(dialog.message).not.toHaveBeenCalled();
  });

  it("re-reads the whole loaded window on refresh instead of one batch", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });
    list(wrapper).vm.$emit("loadMore");
    await flush();
    expect(rows(wrapper)).toHaveLength(100);

    list(wrapper).vm.$emit("refreshAll");
    await flush();

    const calls = listCalls();
    const last = calls[calls.length - 1];
    expect(last.args.limit, "re-reads what is loaded, not a page").toBe(100);
    expect(last.args.after).toBeNull();
    expect(rows(wrapper), "the reader's place is kept").toHaveLength(100);
    expect(list(wrapper).props("hasMore"), "20 entries still below").toBe(true);
  });

  it("reports the filtered total in the unread view", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    expect(list(wrapper).props("view"), "unread is the default").toBe("unread");
    expect(list(wrapper).props("total"), "40 of 120 start read").toBe(80);
    expect(rows(wrapper)).toHaveLength(50);

    list(wrapper).vm.$emit("setView", "all");
    await flush();

    expect(list(wrapper).props("total")).toBe(120);
    expect(rows(wrapper)).toHaveLength(50);
  });
});

describe("reading pane", () => {
  it("keeps the open entry while further batches load", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });
    const open = rows(wrapper)[0];

    list(wrapper).vm.$emit("select", open);
    await flush();
    expect(pane(wrapper).props("entry")?.id).toBe(open.id);

    list(wrapper).vm.$emit("loadMore");
    await flush();

    expect(rows(wrapper)).toHaveLength(100);
    expect(pane(wrapper).props("entry")?.id).toBe(open.id);
  });

  it("keeps the list row and the open entry in step", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    const open = rows(wrapper).find((r) => !r.is_read);
    expect(open, "seed must contain an unread entry").toBeTruthy();

    list(wrapper).vm.$emit("select", open);
    await flush();

    // Selecting marks it read: both copies must agree.
    expect(pane(wrapper).props("entry")?.is_read).toBe(true);
    expect(rowById(wrapper, open!.id).is_read).toBe(true);

    pane(wrapper).vm.$emit("toggleRead");
    await flush();

    expect(pane(wrapper).props("entry")?.is_read).toBe(false);
    expect(rowById(wrapper, open!.id).is_read).toBe(false);
  });

  it("clears the pane when the open entry's feed is removed", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    const open = rows(wrapper).find((r) => r.feed_id === 2)!;
    list(wrapper).vm.$emit("select", open);
    await flush();
    expect(pane(wrapper).props("entry")).not.toBeNull();

    await backend.invoke("remove_feed", { feedId: 2 });
    wrapper.findComponent(Sidebar).vm.$emit("remove", 2);
    await flush();

    expect(pane(wrapper).props("entry")).toBeNull();
    expect(
      rows(wrapper).every((r) => r.feed_id !== 2),
      "the removed feed's rows are gone",
    ).toBe(true);
  });
});

describe("progress", () => {
  it("counts loaded rows against the current view's total", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });
    expect(list(wrapper).props("viewedCount")).toBe(50);

    list(wrapper).vm.$emit("loadMore");
    await flush();

    expect(list(wrapper).props("viewedCount")).toBe(100);
    expect(list(wrapper).props("total")).toBe(120);
  });

  it("does not carry a new-arrival counter any more", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    expect(list(wrapper).props()).not.toHaveProperty("newArrivals");
    expect(wrapper.find(".new-arrivals").exists()).toBe(false);
  });
});

describe("scope reset", () => {
  it("starts over at the newest batch when a feed is selected", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2, view: "all" });
    list(wrapper).vm.$emit("loadMore");
    await flush();
    expect(rows(wrapper)).toHaveLength(100);

    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 1 });
    await flush();

    expect(rows(wrapper), "a fresh single batch").toHaveLength(50);
    expect(rows(wrapper).every((r) => r.feed_id === 1)).toBe(true);
    expect(list(wrapper).props("viewedCount")).toBe(50);
  });
});

describe("row detail wiring", () => {
  it("passes feed titles to the list so rows can name their feed", async () => {
    backend = new FakeBackend({ entries: 4, feeds: 2 });
    backend.feeds[0].title = "Renamed Feed";
    harness.backend = backend;
    const wrapper = mount(App);
    await flush();

    expect(list(wrapper).props("feedTitles")).toMatchObject({
      1: "Renamed Feed",
      2: "Feed 2",
    });

    const labels = wrapper.findAll(".entry-feed").map((n) => n.text());
    expect(labels).toContain("Renamed Feed");
    expect(labels).toContain("Feed 2");
  });

  it("stops showing the feed label once a single feed is selected", async () => {
    const wrapper = await mountApp({ entries: 4, feeds: 2 });
    expect(list(wrapper).props("showFeedLabel")).toBe(true);

    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 1 });
    await flush();

    expect(list(wrapper).props("showFeedLabel")).toBe(false);
    expect(wrapper.find(".entry-feed").exists()).toBe(false);
  });
});

describe("batch read", () => {
  /** Every third seeded entry starts read, so a page holds 33-ish unread. */
  function unreadIn(items: EntryListItem[]): EntryListItem[] {
    return items.filter((r) => !r.is_read);
  }

  async function openFeed(wrapper: VueWrapper, id = 1): Promise<void> {
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id });
    await flush();
  }

  it("is not offered in the unfiltered all-entries view", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });
    expect(list(wrapper).props("batchActionsAvailable")).toBe(false);
    expect(wrapper.find(".mark-rendered").exists()).toBe(false);
    expect(wrapper.find(".mark-above").exists()).toBe(false);
  });

  it("is not offered in the starred view", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "starred" });
    await flush();
    expect(wrapper.find(".mark-rendered").exists()).toBe(false);
    expect(wrapper.find(".mark-above").exists()).toBe(false);
  });

  it("is offered in the global unread view", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    list(wrapper).vm.$emit("setView", "unread");
    await flush();
    expect(wrapper.find(".mark-rendered").exists()).toBe(true);
  });

  it("marks only the page the reader can see", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    await openFeed(wrapper);
    const page = rows(wrapper);
    expect(page).toHaveLength(50);
    const pageUnread = unreadIn(page).length;
    expect(backend.unreadFor(1)).toBe(80);

    await wrapper.find(".mark-rendered").trigger("click");
    await flush();

    expect(backend.unreadFor(1)).toBe(80 - pageUnread);
    // Older entries the page never showed are untouched.
    const olderThanPage = backend.entries.find(
      (e) => e.id < page[page.length - 1].id && !e.is_read,
    );
    expect(olderThanPage, "the seed has unread entries below the page").toBeTruthy();
    expect(olderThanPage!.is_read).toBe(false);
  });

  it("refills the unread page so the reader never has to scroll", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    list(wrapper).vm.$emit("setView", "unread");
    await flush();
    const before = rows(wrapper);
    expect(before).toHaveLength(50);
    const beforeIds = before.map((r) => r.id);

    await wrapper.find(".mark-rendered").trigger("click");
    await flush();

    const after = rows(wrapper);
    expect(after).toHaveLength(80 - before.length);
    expect(list(wrapper).props("total")).toBe(80 - before.length);
    expect(after.every((r) => !r.is_read)).toBe(true);
    expect(after.some((r) => beforeIds.includes(r.id))).toBe(false);
  });

  it("leaves the row the action was used on, and everything below it, alone", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1, view: "all" });
    await openFeed(wrapper);
    const page = rows(wrapper);
    // Snapshot the flags: the app patches the same objects it renders.
    const aboveIds = unreadIn(page.slice(0, 5)).map((r) => r.id);
    const untouchedIds = page.slice(5).filter((r) => !r.is_read).map((r) => r.id);
    expect(aboveIds.length).toBeGreaterThan(0);

    list(wrapper).vm.$emit("markAbove", 5);
    await flush();

    for (const id of aboveIds) {
      expect(backend.entries.find((e) => e.id === id)?.is_read).toBe(true);
    }
    for (const id of untouchedIds) {
      expect(backend.entries.find((e) => e.id === id)?.is_read).toBe(false);
    }
    // A feed's list also shows read entries, so nothing moves.
    expect(rows(wrapper).map((r) => r.id)).toEqual(page.map((r) => r.id));
  });

  it("clears a whole feed from the sidebar, including entries never listed", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    expect(backend.unreadFor(1)).toBe(80);

    const trigger = wrapper
      .findAll(".row-menu-trigger")
      .find((b) => b.attributes("title") === "Feed actions");
    await trigger!.trigger("click");
    const item = wrapper
      .findAll(".row-menu-item")
      .find((b) => b.text().startsWith("Mark all as read"));
    await item!.trigger("click");
    await flush();

    expect(backend.unreadFor(1)).toBe(0);
    expect(
      backend.entries.every((e) => e.is_read),
      "all 120 entries are read although only 50 were ever loaded",
    ).toBe(true);
    expect(wrapper.findAll(".entry.unread")).toHaveLength(0);
  });

  it("updates the open entry when a batch mark covers it", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    list(wrapper).vm.$emit("setView", "unread");
    await flush();
    const open = rows(wrapper)[0];

    list(wrapper).vm.$emit("select", open);
    await flush();
    // Selecting marks it read; flip it back so the batch is what changes it.
    pane(wrapper).vm.$emit("toggleRead");
    await flush();
    expect(pane(wrapper).props("entry")?.is_read).toBe(false);

    await wrapper.find(".mark-rendered").trigger("click");
    await flush();

    expect(pane(wrapper).props("entry")?.is_read).toBe(true);
  });

  it("keeps the open entry in step when the sidebar clears its feed", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    await openFeed(wrapper);
    const open = rows(wrapper).find((r) => !r.is_read)!;
    list(wrapper).vm.$emit("select", open);
    await flush();
    pane(wrapper).vm.$emit("toggleRead");
    await flush();
    expect(pane(wrapper).props("entry")?.is_read).toBe(false);

    const trigger = wrapper
      .findAll(".row-menu-trigger")
      .find((b) => b.attributes("title") === "Feed actions");
    await trigger!.trigger("click");
    const item = wrapper
      .findAll(".row-menu-item")
      .find((b) => b.text().startsWith("Mark all as read"));
    await item!.trigger("click");
    await flush();

    expect(pane(wrapper).props("entry")?.id).toBe(open.id);
    expect(pane(wrapper).props("entry")?.is_read).toBe(true);
  });
});

describe("library-wide batch read", () => {
  async function markEverything(wrapper: VueWrapper): Promise<void> {
    const trigger = wrapper
      .findAll(".row-menu-trigger")
      .find((b) => b.attributes("title") === "All entries actions");
    if (!trigger) throw new Error("the All entries row has no menu");
    await trigger.trigger("click");
    const item = wrapper
      .findAll(".row-menu-item")
      .find((b) => b.text().startsWith("Mark all as read"));
    await item!.trigger("click");
    await flush();
  }

  it("labels the All entries row with the library's unread total", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    expect(wrapper.findComponent(Sidebar).props("unreadCount")).toBe(80);
    expect(backend.unreadFor(1) + backend.unreadFor(2)).toBe(80);
  });

  it("marks everything read from a feed's unread view, unloaded rows included", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 1 });
    await flush();
    list(wrapper).vm.$emit("setView", "unread");
    await flush();
    const open = rows(wrapper)[0];
    list(wrapper).vm.$emit("select", open);
    await flush();

    await markEverything(wrapper);

    expect(
      backend.entries.every((e) => e.is_read),
      "not only feed 1's entries",
    ).toBe(true);
    expect(rows(wrapper), "the unread list is empty").toHaveLength(0);
    expect(pane(wrapper).props("entry")?.is_read, "the open entry was re-read").toBe(true);
    expect(wrapper.findComponent(Sidebar).props("unreadCount")).toBe(0);
  });

  it("offers no library-wide action once nothing is unread", async () => {
    const wrapper = await mountApp({ entries: 6, feeds: 1 });
    await markEverything(wrapper);
    expect(
      wrapper
        .findAll(".row-menu-trigger")
        .some((b) => b.attributes("title") === "All entries actions"),
    ).toBe(false);
  });
});

describe("list filter", () => {
  function sidebar(wrapper: VueWrapper) {
    return wrapper.findComponent(Sidebar);
  }

  it("opens on the unread filter", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    expect(list(wrapper).props("view")).toBe("unread");
    expect(rows(wrapper).every((r) => !r.is_read)).toBe(true);
    expect(list(wrapper).props("total"), "40 of 120 start read").toBe(80);
  });

  it("keeps the unread filter when a feed is selected", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    sidebar(wrapper).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();

    expect(list(wrapper).props("view")).toBe("unread");
    const loaded = rows(wrapper);
    expect(loaded.length).toBeGreaterThan(0);
    expect(loaded.every((r) => r.feed_id === 2 && !r.is_read)).toBe(true);
  });

  it("filters a folder to unread as well", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    backend.feeds[0].folder_id = 9;
    backend.feeds[1].folder_id = 9;
    wrapper
      .findComponent(Sidebar)
      .vm.$emit("select", { kind: "folder", id: 9 });
    await flush();

    const loaded = rows(wrapper);
    expect(loaded.length).toBeGreaterThan(0);
    expect(loaded.every((r) => !r.is_read)).toBe(true);
  });

  it("keeps the feed selected when the All tab is chosen inside it", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    sidebar(wrapper).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();

    list(wrapper).vm.$emit("setView", "all");
    await flush();

    expect(list(wrapper).props("view")).toBe("all");
    const loaded = rows(wrapper);
    expect(loaded.every((r) => r.feed_id === 2), "still the feed's list").toBe(true);
    expect(
      loaded.some((r) => r.is_read),
      "read entries are reachable from inside the feed",
    ).toBe(true);
    expect(sidebar(wrapper).props("selection")).toEqual({ kind: "feed", id: 2 });
  });

  it("carries the chosen filter over to the next selection", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    sidebar(wrapper).vm.$emit("select", { kind: "feed", id: 1 });
    await flush();
    list(wrapper).vm.$emit("setView", "all");
    await flush();

    sidebar(wrapper).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();

    expect(list(wrapper).props("view")).toBe("all");
    expect(
      rows(wrapper).some((r) => r.is_read),
      "the archive view carried over",
    ).toBe(true);
  });

  it("shows every starred entry whatever the filter is", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 1 });
    backend.entries.forEach((e, i) => {
      e.is_starred = i % 4 === 0;
    });

    sidebar(wrapper).vm.$emit("select", { kind: "starred" });
    await flush();

    expect(list(wrapper).props("view")).toBe("all");
    expect(list(wrapper).props("unreadFilterAvailable")).toBe(false);
    expect(wrapper.findAll(".view-tabs button"), "no unread tab").toHaveLength(1);
    expect(rows(wrapper).some((r) => r.is_read)).toBe(true);
  });
});

describe("background sync", () => {
  it("sends no sync command when no connection is configured", async () => {
    await mountApp({ entries: 5 });

    expect(backend.countCalls("load_sync_config")).toBe(1);
    expect(backend.countCalls("sync_now")).toBe(0);
  });

  it("refreshes the view after a sync that applied changes", async () => {
    // Baseline: the same library with no connection, loaded once.
    await mountApp({ entries: 5 });
    const withoutSync = backend.countCalls("list_entries");

    await mountApp({ entries: 5, sync: { outcome: "synced" } });

    expect(backend.countCalls("sync_now")).toBe(1);
    expect(backend.countCalls("list_entries")).toBeGreaterThan(withoutSync);
  });

  it("leaves the view alone after a sync that changed nothing", async () => {
    await mountApp({ entries: 5, sync: { outcome: "nothing_changed" } });
    await flush();
    const lists = backend.countCalls("list_entries");

    expect(backend.countCalls("sync_now")).toBe(1);
    expect(backend.countCalls("list_entries")).toBe(lists);
  });

  it("syncs again when the window regains focus", async () => {
    await mountApp({ entries: 5, sync: { outcome: "nothing_changed" } });
    const before = backend.countCalls("sync_now");

    window.dispatchEvent(new Event("focus"));
    await flush();

    // Apps mounted by earlier tests are still listening on the same window, so
    // what matters is that focus causes another sync, not the exact count.
    expect(backend.countCalls("sync_now")).toBeGreaterThan(before);
  });

  it("shows a download-only sync's changes without a restart", async () => {
    const wrapper = await mountApp({
      entries: 120,
      feeds: 1,
      view: "all",
      sync: { outcome: "nothing_changed", applied: true },
    });
    list(wrapper).vm.$emit("loadMore");
    await flush();
    expect(rows(wrapper)).toHaveLength(100);
    const before = {
      feeds: backend.countCalls("list_feeds"),
      folders: backend.countCalls("list_folders"),
      counts: backend.countCalls("entry_counts"),
      rows: backend.countCalls("list_entries"),
    };

    // The document brought a subscription this installation had never fetched.
    backend.feeds.push({
      id: 2,
      url: "https://f2.example/feed",
      title: "From the other installation",
      site_url: null,
      last_fetched_at: null,
      last_error: null,
      folder_id: null,
      unread_count: 0,
    });
    window.dispatchEvent(new Event("focus"));
    await flush();

    const sidebar = wrapper.findComponent(Sidebar);
    expect(
      (sidebar.props("feeds") as { url: string }[]).map((f) => f.url),
      "the sidebar lists the synced subscription without a restart",
    ).toContain("https://f2.example/feed");
    expect(backend.countCalls("list_feeds")).toBeGreaterThan(before.feeds);
    expect(backend.countCalls("list_folders")).toBeGreaterThan(before.folders);
    expect(backend.countCalls("entry_counts")).toBeGreaterThan(before.counts);
    expect(backend.countCalls("list_entries")).toBeGreaterThan(before.rows);
    expect(rows(wrapper), "the loaded window is kept").toHaveLength(100);
    expect(list(wrapper).props("hasMore"), "the reader's place is kept").toBe(true);
    expect(list(wrapper).props("view"), "the filter is kept").toBe("all");
    expect(sidebar.props("selection"), "the selection is kept").toEqual({ kind: "all" });
  });
});
