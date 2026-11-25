import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App.vue";
import EntryList from "../components/EntryList.vue";
import ReadingPane from "../components/ReadingPane.vue";
import Sidebar from "../components/Sidebar.vue";
import type { EntryListItem } from "../api";
import { BINDINGS } from "../lib/keymap";
import { resetKeyboardBlocks } from "../lib/keyboard-blocks";
import {
  FakeBackend,
  FakeIntersectionObserver,
  flush,
  stubGrowingListGeometry,
  stubScrollGeometry,
} from "./fakeBackend";

/** The IPC bridge the mocked `@tauri-apps/api/core` dispatches into, plus the
 *  opener, so an `o` press can be observed. */
const harness = vi.hoisted(() => ({
  backend: {
    invoke: (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> =>
      Promise.resolve(null),
  },
  openUrl: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) =>
    harness.backend.invoke(cmd, args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: async () => undefined,
  confirm: async () => true,
  open: async () => null,
  save: async () => null,
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: (url: string) => harness.openUrl(url),
}));

/** Longer than the burst flush delay, so a burst has landed. */
const BURST_SETTLE_MS = 160;

let backend: FakeBackend;
/** The mounted app, so a test cannot leave a listener behind for the next one:
 *  the container listens on `document`, and a leaked instance would react to
 *  every later keystroke. */
let mounted: VueWrapper | null = null;

async function mountApp(
  options: { entries?: number; feeds?: number; view?: "all" | "unread" } = {},
): Promise<VueWrapper> {
  backend = new FakeBackend(options);
  harness.backend = backend;
  const wrapper = mount(App, { attachTo: document.body });
  mounted = wrapper;
  await flush();
  if (options.view === "all") {
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
  if (!found) throw new Error(`entry ${id} is not loaded`);
  return found;
}

function cursorId(wrapper: VueWrapper): number | null {
  return list(wrapper).props("selectedEntryId") as number | null;
}

function cursorIndex(wrapper: VueWrapper): number {
  const id = cursorId(wrapper);
  return id === null ? -1 : rows(wrapper).findIndex((r) => r.id === id);
}

/** Dispatch a keystroke at `document`, the way the container listens for it. */
function press(
  key: string,
  options: { ctrl?: boolean; shift?: boolean; target?: EventTarget } = {},
): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    key,
    ctrlKey: options.ctrl ?? false,
    shiftKey: options.shift ?? false,
    bubbles: true,
    cancelable: true,
  });
  (options.target ?? document).dispatchEvent(event);
  return event;
}

/** Press every key in order, then let the render settle. */
async function keys(...sequence: string[]): Promise<void> {
  for (const key of sequence) press(key);
  await flush();
}

/** Wait out the burst flush, then let the resulting requests settle. */
async function settleBurst(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, BURST_SETTLE_MS));
  await flush();
}

function entryCalls(): number {
  return backend.countCalls("get_entry");
}

function markCalls(): number {
  return backend.countCalls("mark_read_many");
}

beforeEach(() => {
  mounted = null;
  resetKeyboardBlocks();
  harness.openUrl.mockClear();
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
  Object.defineProperty(Element.prototype, "scrollIntoView", {
    value: vi.fn(),
    writable: true,
    configurable: true,
  });
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("moving the cursor", () => {
  it("opens the first row and marks it read", async () => {
    const wrapper = await mountApp({ entries: 12 });
    expect(cursorId(wrapper)).toBeNull();

    await keys("j");

    const opened = cursorId(wrapper)!;
    expect(opened).toBe(rows(wrapper)[0].id);
    expect(cursorIndex(wrapper)).toBe(0);
    expect(rowById(wrapper, opened).is_read, "the row is read at once").toBe(true);
    await settleBurst();
    expect(backend.entries.find((e) => e.id === opened)?.is_read).toBe(true);
  });

  it("walks down and does not unmark what it leaves behind", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("j", "j");
    const left = cursorId(wrapper)!;

    await keys("k");

    expect(cursorIndex(wrapper)).toBe(0);
    expect(rowById(wrapper, left).is_read, "the row it left stays read").toBe(true);
    await settleBurst();
    expect(backend.entries.find((e) => e.id === left)?.is_read).toBe(true);
  });

  it("stays put at the top and at the end of the loaded rows", async () => {
    const wrapper = await mountApp({ entries: 3, view: "all" });
    await keys("k");
    expect(cursorId(wrapper)).toBeNull();

    await keys("j", "j", "j");
    await settleBurst();
    expect(cursorIndex(wrapper)).toBe(2);

    const before = entryCalls();
    press("j");
    await flush();
    expect(cursorIndex(wrapper), "the last row holds").toBe(2);
    expect(entryCalls(), "no row is opened again").toBe(before);
  });

  it("does nothing on an empty list", async () => {
    const wrapper = await mountApp({ entries: 0 });
    await keys("j", "j", "G");
    expect(cursorId(wrapper)).toBeNull();
    expect(entryCalls()).toBe(0);
  });

  it("continues from the row the mouse clicked", async () => {
    const wrapper = await mountApp({ entries: 12, view: "all" });
    const third = rows(wrapper)[2];
    await wrapper.findAll(".entry")[2].trigger("click");
    await flush();
    expect(cursorId(wrapper)).toBe(third.id);

    await keys("j");
    expect(cursorId(wrapper)).toBe(rows(wrapper)[3].id);
  });
});

describe("motions and counts", () => {
  it("counts steps", async () => {
    const wrapper = await mountApp({ entries: 30 });
    await keys("j");
    press("5");
    await keys("j");
    expect(cursorIndex(wrapper)).toBe(5);
  });

  it("stops a large count at the last row", async () => {
    const wrapper = await mountApp({ entries: 10, view: "all" });
    await keys("j");
    press("2");
    press("0");
    await keys("j");
    expect(cursorIndex(wrapper)).toBe(9);
  });

  it("jumps to the top with gg and to the end with G", async () => {
    const wrapper = await mountApp({ entries: 30, view: "all" });
    await keys("G");
    expect(cursorIndex(wrapper)).toBe(29);

    await keys("g", "g");
    expect(cursorIndex(wrapper)).toBe(0);
  });

  it("reaches a numbered row with a count and G", async () => {
    const wrapper = await mountApp({ entries: 30, view: "all" });
    press("1");
    press("2");
    await keys("G");
    expect(cursorIndex(wrapper)).toBe(11);
  });

  it("moves half a screen with Ctrl-d and Ctrl-u", async () => {
    const wrapper = await mountApp({ entries: 30, view: "all" });
    const box = wrapper.find(".entries").element as HTMLElement;
    const row = wrapper.find(".entry").element as HTMLElement;
    Object.defineProperty(box, "clientHeight", { value: 400, configurable: true });
    Object.defineProperty(row, "offsetHeight", { value: 100, configurable: true });

    await keys("j");
    const event = press("d", { ctrl: true });
    await flush();
    expect(cursorIndex(wrapper), "four visible rows means two steps").toBe(2);
    expect(event.defaultPrevented, "the webview must not scroll too").toBe(true);

    press("u", { ctrl: true });
    await flush();
    expect(cursorIndex(wrapper)).toBe(0);
  });

  it("scrolls the cursor row into view", async () => {
    await mountApp({ entries: 30, view: "all" });
    await keys("G");
    const spy = Element.prototype.scrollIntoView as unknown as ReturnType<typeof vi.fn>;
    expect(spy).toHaveBeenCalled();
  });
});

describe("the g prefix", () => {
  it("switches to the starred view", async () => {
    const wrapper = await mountApp({ entries: 12 });
    backend.entries[0].is_starred = true;

    await keys("g", "s");

    expect(wrapper.findComponent(Sidebar).props("selection")).toEqual({ kind: "starred" });
    expect(rows(wrapper).map((r) => r.id)).toEqual([1]);
  });

  it("switches to all entries from a feed", async () => {
    const wrapper = await mountApp({ entries: 12, feeds: 2 });
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();

    await keys("g", "a");

    expect(wrapper.findComponent(Sidebar).props("selection")).toEqual({ kind: "all" });
  });

  it("drops a prefix that leads nowhere without eating the key", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("g", "j");
    expect(cursorIndex(wrapper), "the j still moved the cursor").toBe(0);
  });

  it("forgets an incomplete sequence after the timeout", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("j");
    press("g");
    await new Promise((resolve) => setTimeout(resolve, 1050));
    await flush();
    // A stray `j` now moves one row, not from some remembered prefixed state.
    await keys("j");
    expect(cursorIndex(wrapper)).toBe(1);
  });
});

describe("the sidebar", () => {
  it("has no keyboard navigation of its own", async () => {
    const wrapper = await mountApp({ entries: 0, feeds: 3 });
    const before = wrapper.findComponent(Sidebar).props("selection");

    await keys("j", "k", "G", "g", "g", "g", "s", "g", "a");

    expect(wrapper.findComponent(Sidebar).props("selection"), "the two jumps only").toEqual(
      before,
    );
  });

  it("switches scope with the jumps even when the list is empty", async () => {
    const wrapper = await mountApp({ entries: 0, feeds: 3 });

    await keys("g", "s");

    expect(wrapper.findComponent(Sidebar).props("selection")).toEqual({ kind: "starred" });
  });
});

describe("entry and list actions", () => {
  it("toggles read state on the cursor row", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("j");
    const id = cursorId(wrapper)!;
    expect(rowById(wrapper, id).is_read).toBe(true);

    await keys("m");
    expect(backend.entries.find((e) => e.id === id)?.is_read).toBe(false);
    expect(wrapper.findComponent(Sidebar).props("unreadCount")).toBe(
      backend.entries.filter((e) => !e.is_read).length,
    );

    await keys("m");
    expect(backend.entries.find((e) => e.id === id)?.is_read).toBe(true);
  });

  it("stars the cursor row", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("j");
    const id = cursorId(wrapper)!;

    await keys("s");

    expect(backend.entries.find((e) => e.id === id)?.is_starred).toBe(true);
    expect(wrapper.findComponent(Sidebar).props("starredCount")).toBe(1);
  });

  it("opens the original article", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("j");
    const link = rows(wrapper)[0].link;
    await keys("o");
    expect(harness.openUrl).toHaveBeenCalledWith(link);
  });

  it("switches the tab and keeps the sidebar selection", async () => {
    const wrapper = await mountApp({ entries: 12 });
    expect(list(wrapper).props("view")).toBe("unread");

    await keys("v");

    expect(list(wrapper).props("view")).toBe("all");
    expect(wrapper.findComponent(Sidebar).props("selection")).toEqual({ kind: "all" });
  });

  it("does nothing in the starred view, which has no filter", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("g", "s");

    await keys("v");

    expect(list(wrapper).props("view")).toBe("all");
    expect(wrapper.findComponent(Sidebar).props("selection")).toEqual({ kind: "starred" });
  });

  it("marks the rendered page read and refills an unread list", async () => {
    const wrapper = await mountApp({ entries: 120 });
    const page = rows(wrapper);
    const pageUnread = page.filter((r) => !r.is_read).length;
    expect(pageUnread).toBeGreaterThan(0);

    await keys("A");

    expect(backend.unreadFor(1)).toBe(80 - pageUnread);
    const after = rows(wrapper);
    expect(after.length, "the page refills").toBeGreaterThan(0);
    expect(after.every((r) => !r.is_read)).toBe(true);
  });

  it("refreshes all feeds and keeps the loaded rows", async () => {
    const wrapper = await mountApp({ entries: 30, view: "all" });
    const before = rows(wrapper).map((r) => r.id);

    await keys("r");

    expect(backend.countCalls("refresh_all")).toBe(1);
    expect(rows(wrapper).map((r) => r.id)).toEqual(before);
  });

  it("ignores row actions while no entry is open", async () => {
    await mountApp({ entries: 12 });
    await keys("m", "s", "o");
    expect(backend.countCalls("set_read")).toBe(0);
    expect(backend.countCalls("set_starred")).toBe(0);
    expect(harness.openUrl).not.toHaveBeenCalled();
  });
});

describe("the reading pane", () => {
  async function paneContent(wrapper: VueWrapper, height: number) {
    const el = wrapper.find(".content").element as HTMLElement;
    Object.defineProperty(el, "clientHeight", { value: height, configurable: true });
    Object.defineProperty(el, "scrollTop", {
      value: 0,
      writable: true,
      configurable: true,
    });
    return el;
  }

  it("scrolls the article without moving the cursor", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("j");
    await settleBurst();
    const content = await paneContent(wrapper, 300);
    const id = cursorId(wrapper);

    await keys("J");
    expect(content.scrollTop).toBe(300);
    expect(cursorId(wrapper)).toBe(id);

    await keys("K");
    expect(content.scrollTop).toBe(0);
  });

  it("starts a newly opened entry at the top", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("j");
    await settleBurst();
    const content = await paneContent(wrapper, 300);
    await keys("J");
    expect(content.scrollTop).toBe(300);

    await keys("j");
    await settleBurst();

    expect(content.scrollTop).toBe(0);
    expect(pane(wrapper).props("entry")?.id).toBe(cursorId(wrapper));
  });
});

describe("bursts", () => {
  it("coalesces content fetches and read marks", async () => {
    const wrapper = await mountApp({ entries: 120, view: "all" });
    const before = { entries: entryCalls(), marks: markCalls() };

    press("j");
    press("j");
    press("j");
    press("j");
    press("j");
    await flush();
    expect(cursorIndex(wrapper), "the cursor keeps up with the keys").toBe(4);
    expect(entryCalls(), "nothing is fetched mid-burst").toBe(before.entries);
    expect(markCalls(), "nothing is written mid-burst").toBe(before.marks);

    await settleBurst();

    expect(entryCalls() - before.entries, "one fetch for the settled row").toBe(1);
    expect(markCalls() - before.marks, "one write for the whole burst").toBe(1);
    expect(backend.entries.filter((e) => !e.is_read && e.id >= 116)).toHaveLength(0);
    expect(pane(wrapper).props("entry")?.id).toBe(cursorId(wrapper));
  });

  it("marks every row the cursor landed on, not only the last", async () => {
    const wrapper = await mountApp({ entries: 40 });
    press("j");
    press("j");
    press("j");
    await settleBurst();

    for (const row of rows(wrapper).slice(0, 3)) {
      expect(backend.entries.find((e) => e.id === row.id)?.is_read, `row ${row.id}`).toBe(
        true,
      );
    }
    expect(cursorIndex(wrapper)).toBe(2);
  });

  it("does not reflow an unread list while the cursor walks down", async () => {
    const wrapper = await mountApp({ entries: 120 });
    const before = rows(wrapper).map((r) => r.id);

    for (let i = 0; i < 8; i++) press("j");
    await settleBurst();

    expect(rows(wrapper).map((r) => r.id), "no row moved or vanished").toEqual(before);
    expect(rows(wrapper).slice(0, 8).every((r) => r.is_read)).toBe(true);
  });

  it("writes a pending burst before the list is replaced", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    press("j");
    press("j");
    const passed = rows(wrapper).slice(0, 2).map((r) => r.id);

    // Switch scope in the same tick: the burst has not been flushed by time.
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();
    await settleBurst();

    for (const id of passed) {
      expect(backend.entries.find((e) => e.id === id)?.is_read, `row ${id}`).toBe(true);
    }
  });

  it("writes a pending burst before the tab changes", async () => {
    const wrapper = await mountApp({ entries: 120 });
    // The first row of the unread list is where the cursor lands.
    const target = rows(wrapper)[0].id;
    press("j");

    list(wrapper).vm.$emit("setView", "all");
    await flush();
    await settleBurst();

    expect(backend.entries.find((e) => e.id === target)?.is_read).toBe(true);
  });

  it("never paints an entry the cursor has already left", async () => {
    const wrapper = await mountApp({ entries: 12, view: "all" });
    const first = rows(wrapper)[0];
    const gate = backend.holdNext("get_entry");
    await wrapper.findAll(".entry")[0].trigger("click");
    await flush();

    await keys("j");
    await settleBurst();
    const settled = cursorId(wrapper)!;
    expect(settled).not.toBe(first.id);

    gate.release();
    await flush();

    expect(pane(wrapper).props("entry")?.id, "the abandoned row stays out").toBe(settled);
  });
});

describe("reloads and paging", () => {
  it("keeps the cursor on its entry across a refresh", async () => {
    const wrapper = await mountApp({ entries: 120, view: "all" });
    press("j");
    press("2");
    press("0");
    press("j");
    await settleBurst();
    const id = cursorId(wrapper)!;
    expect(cursorIndex(wrapper)).toBe(20);

    await wrapper.find(".refresh-all").trigger("click");
    await flush();

    expect(cursorId(wrapper)).toBe(id);
    expect(cursorIndex(wrapper)).toBe(20);
  });

  it("keeps the cursor on its entry when a batch is appended", async () => {
    const wrapper = await mountApp({ entries: 120, view: "all" });
    await keys("j");
    const id = cursorId(wrapper)!;

    list(wrapper).vm.$emit("loadMore");
    await flush();

    expect(cursorId(wrapper)).toBe(id);
  });

  it("stands in at the same position when its entry disappears", async () => {
    const wrapper = await mountApp({ entries: 120, view: "all" });
    press("j");
    press("5");
    press("j");
    await settleBurst();
    const open = cursorId(wrapper)!;
    expect(cursorIndex(wrapper)).toBe(5);

    // The entry is gone by the time the refresh re-reads it.
    const stale = backend.entries.findIndex((e) => e.id === open);
    backend.entries.splice(stale, 1);

    await wrapper.find(".refresh-all").trigger("click");
    await flush();

    expect(cursorId(wrapper)).not.toBe(open);
    expect(cursorIndex(wrapper), "the position is kept").toBe(5);
    expect(pane(wrapper).props("entry")?.id).toBe(cursorId(wrapper));
  });

  it("clears the cursor when the reader removes the open entry's feed", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2, view: "all" });
    // Step onto a row that belongs to feed 2, and let it land.
    const target = rows(wrapper).findIndex((r) => r.feed_id === 2);
    for (let step = 0; step <= target; step++) press("j");
    await settleBurst();
    const open = cursorId(wrapper)!;
    expect(backend.entries.find((e) => e.id === open)?.feed_id).toBe(2);

    wrapper.findComponent(Sidebar).vm.$emit("remove", 2);
    await flush();

    // Existing behaviour, kept: removing a subscription takes the reader out of
    // the entry rather than silently swapping in a neighbour.
    expect(cursorId(wrapper)).toBeNull();
    expect(pane(wrapper).props("entry")).toBeNull();
    expect(rows(wrapper).length, "the rest of the list stays").toBeGreaterThan(0);
    expect(rows(wrapper).every((r) => r.feed_id !== 2)).toBe(true);
  });

  it("clears the cursor when the selection changes", async () => {
    const wrapper = await mountApp({ entries: 120, feeds: 2 });
    await keys("j");
    expect(cursorId(wrapper)).not.toBeNull();

    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();

    expect(cursorId(wrapper)).toBeNull();
    expect(pane(wrapper).props("entry")).toBeNull();
  });

  it("continues on the next batch when it walks off the loaded rows", async () => {
    const wrapper = await mountApp({ entries: 120, view: "all" });
    for (let i = 0; i < 50; i++) press("j");
    await flush();
    expect(cursorIndex(wrapper)).toBe(49);
    const loaded = rows(wrapper).length;

    press("j");
    await flush();
    await flush();

    expect(rows(wrapper).length, "a batch was appended").toBeGreaterThan(loaded);
    expect(cursorIndex(wrapper), "the move carried over").toBe(50);
  });

  it("stays on the last row when the next batch is empty", async () => {
    const wrapper = await mountApp({ entries: 120, view: "all" });
    for (let i = 0; i < 50; i++) press("j");
    await flush();
    const last = cursorId(wrapper)!;
    // Everything below the loaded rows is gone before the next batch.
    const loadedIds = rows(wrapper).map((r) => r.id);
    backend.entries.splice(
      0,
      backend.entries.length,
      ...backend.entries.filter((e) => loadedIds.includes(e.id)),
    );

    await keys("j");
    await flush();

    expect(cursorId(wrapper)).toBe(last);
    expect(list(wrapper).props("hasMore")).toBe(false);
  });

  it("jumps to the end of the loaded rows without loading the rest", async () => {
    const wrapper = await mountApp({ entries: 3000, view: "all" });
    const before = backend.countCalls("list_entries");

    await keys("G");

    expect(cursorIndex(wrapper), "the cursor moves on the keypress").toBe(49);
    expect(rows(wrapper), "no batch was fetched to get there").toHaveLength(50);
    expect(backend.countCalls("list_entries")).toBe(before);
  });

  it("does not chase a second batch when one already fills the list", async () => {
    stubGrowingListGeometry();
    const wrapper = await mountApp({ entries: 3000, view: "all" });
    await keys("G");
    expect(cursorIndex(wrapper)).toBe(49);

    // Revealing the last row puts the sentinel in view, which is the ordinary
    // "reached the end" request: exactly one batch, not one per render.
    FakeIntersectionObserver.last().trigger(true);
    await flush();

    expect(rows(wrapper), "one batch, not two").toHaveLength(100);
  });

  it("extends the list one batch per press once it is at the end", async () => {
    const wrapper = await mountApp({ entries: 3000, view: "all" });
    await keys("G");
    expect(cursorIndex(wrapper)).toBe(49);

    await keys("G");
    await flush();
    expect(rows(wrapper)).toHaveLength(100);
    expect(cursorIndex(wrapper), "the new end of the list").toBe(99);

    await keys("G");
    await flush();
    expect(rows(wrapper)).toHaveLength(150);
    expect(cursorIndex(wrapper)).toBe(149);
  });

  it("stays at the end when no further entries match", async () => {
    const wrapper = await mountApp({ entries: 60, view: "all" });
    await keys("G");
    expect(cursorIndex(wrapper)).toBe(49);

    await keys("G");
    await flush();
    expect(cursorIndex(wrapper)).toBe(59);

    const before = backend.countCalls("list_entries");
    await keys("G");
    expect(cursorIndex(wrapper)).toBe(59);
    expect(backend.countCalls("list_entries")).toBe(before);
  });
});

describe("gating", () => {
  it("types into a dialog field instead of acting on the list", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await wrapper.find(".settings-btn").trigger("click");
    await flush();

    const input = document.querySelector("input");
    expect(input, "the settings dialog has a field").toBeTruthy();
    const before = entryCalls();
    press("j", { target: input! });
    press("r");
    await flush();

    expect(cursorId(wrapper), "no dialog key leaked out").toBeNull();
    expect(entryCalls()).toBe(before);
    expect(backend.countCalls("refresh_all")).toBe(0);
  });

  it("does not act while a row menu is open", async () => {
    const wrapper = await mountApp({ entries: 12, feeds: 1 });
    await wrapper.find(".row-menu-trigger").trigger("click");
    await flush();

    await keys("j", "A", "r");

    expect(cursorId(wrapper)).toBeNull();
    expect(backend.countCalls("refresh_all")).toBe(0);
    expect(markCalls()).toBe(0);
  });

  it("closes a row menu with Escape and then acts again", async () => {
    const wrapper = await mountApp({ entries: 12, feeds: 1 });
    await wrapper.find(".row-menu-trigger").trigger("click");
    await flush();

    await keys("Escape");
    await keys("j");

    expect(wrapper.find(".row-menu-panel").exists()).toBe(false);
    expect(cursorIndex(wrapper)).toBe(0);
  });

  it("leaves modified keys to the webview", async () => {
    const wrapper = await mountApp({ entries: 12 });
    const event = press("j", { ctrl: true });
    await flush();
    expect(cursorId(wrapper)).toBeNull();
    expect(event.defaultPrevented).toBe(false);
  });
});

describe("the help overlay", () => {
  it("lists every binding", async () => {
    await mountApp({ entries: 12 });
    await keys("?");

    // The overlay is teleported to the document, outside the wrapper's tree.
    const shown = Array.from(document.querySelectorAll(".help-keys kbd")).map(
      (n) => n.textContent?.trim() ?? "",
    );
    expect(shown).toHaveLength(BINDINGS.length);
    for (const binding of BINDINGS) {
      expect(shown, binding.label).toContain(binding.label);
    }
  });

  it("closes on ? and on Escape", async () => {
    await mountApp({ entries: 12 });
    await keys("?");
    expect(document.querySelector(".help-overlay")).toBeTruthy();
    await keys("?");
    expect(document.querySelector(".help-overlay")).toBeNull();

    await keys("?");
    expect(document.querySelector(".help-overlay")).toBeTruthy();
    await keys("Escape");
    expect(document.querySelector(".help-overlay")).toBeNull();
  });

  it("does not let keys act through it", async () => {
    const wrapper = await mountApp({ entries: 12 });
    await keys("?");

    await keys("j", "r", "A");

    expect(cursorId(wrapper)).toBeNull();
    expect(backend.countCalls("refresh_all")).toBe(0);
    expect(markCalls()).toBe(0);
  });
});
