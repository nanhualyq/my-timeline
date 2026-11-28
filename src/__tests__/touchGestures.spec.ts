import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import App from "../App.vue";
import EntryList from "../components/EntryList.vue";
import ReadingPane from "../components/ReadingPane.vue";
import TouchMenu from "../components/TouchMenu.vue";
import Sidebar from "../components/Sidebar.vue";
import UndoBar from "../components/UndoBar.vue";
import {
  FakeBackend,
  FakeIntersectionObserver,
  flush,
  installViewport,
  stubScrollGeometry,
  type InstalledViewport,
} from "./fakeBackend";

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

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: async () => undefined,
  confirm: async () => true,
  open: async () => null,
  save: async () => null,
}));

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => undefined }));

let viewport: InstalledViewport;
let backend: FakeBackend;

/**
 * jsdom has no PointerEvent. The gesture module only reads `button`,
 * `clientX`, `clientY`, and `currentTarget`, all of which MouseEvent carries,
 * so a MouseEvent named `pointerdown` drives exactly the same path.
 */
function press(target: EventTarget, type: string, x: number, y: number) {
  target.dispatchEvent(
    new MouseEvent(type, {
      bubbles: true,
      cancelable: true,
      clientX: x,
      clientY: y,
    }),
  );
}

/** A drag that starts on a row and ends `dx` pixels away. */
async function dragRow(row: Element, dx: number) {
  press(row, "pointerdown", 300, 300);
  press(window, "pointermove", 300 + dx, 302);
  press(window, "pointermove", 300 + dx, 302);
  press(window, "pointerup", 300 + dx, 302);
  await nextTick();
}

async function mountApp(): Promise<VueWrapper> {
  backend = new FakeBackend({ entries: 12, feeds: 1 });
  harness.backend = backend;
  const wrapper = mount(App, { attachTo: document.body });
  await flush();
  return wrapper;
}

function list(wrapper: VueWrapper) {
  return wrapper.findComponent(EntryList);
}

function pane(wrapper: VueWrapper) {
  return wrapper.findComponent(ReadingPane);
}

/** The id of the entry the reading pane is showing. Fails loudly when nothing
 *  is open, which is what a broken swipe would look like. */
function openEntryId(wrapper: VueWrapper): number {
  const entry = pane(wrapper).props("entry") as { id: number } | null;
  if (entry === null) throw new Error("no entry is open");
  return entry.id;
}

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
  // A phone: narrow, coarse pointer, no hovering.
  viewport = installViewport({ narrow: true, coarseNarrow: true, coarse: true });
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete document.documentElement.dataset.layout;
  delete document.documentElement.dataset.input;
});

describe("row drags", () => {
  it("stars an entry dragged leftward", async () => {
    const wrapper = await mountApp();
    const row = wrapper.findAll(".entry")[1];
    const id = Number(row.attributes("data-entry-id"));

    await dragRow(row.element, -160);

    expect(backend.countCalls("set_starred")).toBe(1);
    expect(backend.entries.find((e) => e.id === id)?.is_starred).toBe(true);
    expect(list(wrapper).props("entries")[1].is_starred).toBe(true);
  });

  it("unstars an entry dragged leftward twice", async () => {
    const wrapper = await mountApp();
    const row = wrapper.findAll(".entry")[1];
    const id = Number(row.attributes("data-entry-id"));

    await dragRow(row.element, -160);
    expect(backend.entries.find((e) => e.id === id)?.is_starred).toBe(true);

    await dragRow(wrapper.findAll(".entry")[1].element, -160);
    expect(backend.entries.find((e) => e.id === id)?.is_starred).toBe(false);
  });

  it("marks the entries above a row dragged rightward", async () => {
    const wrapper = await mountApp();
    const rows = wrapper.findAll(".entry");
    const above = rows.slice(0, 2).map((r) => Number(r.attributes("data-entry-id")));
    expect(above.every((id) => !backend.entries.find((e) => e.id === id)?.is_read)).toBe(
      true,
    );

    await dragRow(rows[2].element, 160);
    await flush();

    for (const id of above) {
      expect(backend.entries.find((e) => e.id === id)?.is_read).toBe(true);
    }
    // And the reader can take it back.
    expect(wrapper.findComponent(UndoBar).exists()).toBe(true);
  });

  it("leaves the entry closed when a drag commits", async () => {
    const wrapper = await mountApp();

    await dragRow(wrapper.findAll(".entry")[1].element, -160);

    expect(pane(wrapper).props("entry")).toBeNull();
    expect(list(wrapper).isVisible()).toBe(true);
  });

  it("opens the entry on a tap", async () => {
    const wrapper = await mountApp();
    const row = wrapper.findAll(".entry")[1];

    press(row.element, "pointerdown", 300, 300);
    press(window, "pointerup", 300, 300);
    await row.trigger("click");
    await flush();

    expect(pane(wrapper).props("entry")).not.toBeNull();
  });

  it("commits nothing for a drag that stops short", async () => {
    const wrapper = await mountApp();

    await dragRow(wrapper.findAll(".entry")[1].element, -40);

    expect(backend.countCalls("set_starred")).toBe(0);
  });

  it("shows the panel that matches the direction of the drag", async () => {
    const wrapper = await mountApp();
    const row = wrapper.findAll(".entry")[2];

    press(row.element, "pointerdown", 300, 300);
    press(window, "pointermove", 200, 300);
    await nextTick();

    expect(row.find(".swipe-panel.trailing").classes()).toContain("revealed");
    expect(row.find(".swipe-panel.leading").classes()).not.toContain("revealed");

    press(window, "pointermove", 400, 300);
    await nextTick();

    // The finger changed direction: so did the offer.
    expect(row.find(".swipe-panel.leading").classes()).toContain("revealed");
    expect(row.find(".swipe-panel.trailing").classes()).not.toContain("revealed");

    press(window, "pointerup", 400, 300);
    await nextTick();
  });

  it("does not act on a pointer layout", async () => {
    viewport.set({ narrow: false, coarseNarrow: false, coarse: false });
    const wrapper = await mountApp();

    await dragRow(wrapper.findAll(".entry")[1].element, -160);

    expect(backend.countCalls("set_starred")).toBe(0);
  });
});

describe("long-press menu", () => {
  async function openMenu(wrapper: VueWrapper, index = 2): Promise<void> {
    // Timers only long enough to make the press count as a long one; the
    // render that follows waits on real ones, because the scheduler cannot
    // drain a faked microtask queue.
    vi.useFakeTimers();
    const row = wrapper.findAll(".entry")[index];
    press(row.element, "pointerdown", 300, 300);
    vi.advanceTimersByTime(600);
    vi.useRealTimers();
    await nextTick();
  }

  it("offers both row actions", async () => {
    const wrapper = await mountApp();
    await openMenu(wrapper);

    const menu = wrapper.findComponent(TouchMenu);
    expect(menu.exists()).toBe(true);
    const labels = menu.findAll(".touch-menu-item").map((item) => item.text());
    expect(labels).toEqual(["Star entry", "Mark 2 entries above read"]);
    expect(menu.get(".touch-menu-title").text()).toContain("Entry");
  });

  it("stars from the menu without opening the entry", async () => {
    const wrapper = await mountApp();
    await openMenu(wrapper);

    await wrapper.get(".touch-menu-item").trigger("click");
    await flush();

    expect(backend.countCalls("set_starred")).toBe(1);
    expect(pane(wrapper).props("entry")).toBeNull();
    expect(wrapper.findComponent(TouchMenu).exists()).toBe(false);
  });

  it("marks the entries above from the menu", async () => {
    const wrapper = await mountApp();
    await openMenu(wrapper);

    await wrapper.findAll(".touch-menu-item")[1].trigger("click");
    await flush();

    expect(backend.countCalls("mark_read_many")).toBe(1);
    expect(wrapper.findComponent(UndoBar).exists()).toBe(true);
  });

  it("changes nothing when dismissed", async () => {
    const wrapper = await mountApp();
    await openMenu(wrapper);

    await wrapper.get(".touch-menu-cancel").trigger("click");
    await nextTick();

    expect(wrapper.findComponent(TouchMenu).exists()).toBe(false);
    expect(backend.countCalls("set_starred")).toBe(0);
    expect(backend.calls.filter((c) => c.cmd === "mark_read_many")).toEqual([]);
  });

  it("closes on the system back action", async () => {
    const wrapper = await mountApp();
    await openMenu(wrapper);
    expect(wrapper.findComponent(TouchMenu).exists()).toBe(true);

    window.dispatchEvent(new PopStateEvent("popstate"));
    await nextTick();

    expect(wrapper.findComponent(TouchMenu).exists()).toBe(false);
  });

  it("does not open on a pointer layout", async () => {
    viewport.set({ narrow: false, coarseNarrow: false, coarse: false });
    const wrapper = await mountApp();

    vi.useFakeTimers();
    press(wrapper.findAll(".entry")[2].element, "pointerdown", 300, 300);
    vi.advanceTimersByTime(600);
    vi.useRealTimers();
    await nextTick();

    expect(wrapper.findComponent(TouchMenu).exists()).toBe(false);
  });
});

describe("reading pane", () => {
  /** Open the row at `index` the way a reader does: tap it. */
  async function openRow(wrapper: VueWrapper, index: number) {
    await wrapper.findAll(".entry")[index].trigger("click");
    await flush();
  }

  /** Longer than the cursor burst's write delay: the article that follows a
   *  swipe arrives with the coalesced write, not with the keypress. */
  async function settleBurst(): Promise<void> {
    await new Promise((resolve) => setTimeout(resolve, 180));
    await flush();
  }

  async function swipe(wrapper: VueWrapper, dx: number) {
    const content = wrapper.get(".content");
    press(content.element, "pointerdown", 300, 300);
    press(window, "pointermove", 300 + dx, 302);
    press(window, "pointerup", 300 + dx, 302);
    await settleBurst();
  }

  it("shows and toggles the open entry's star", async () => {
    const wrapper = await mountApp();
    await openRow(wrapper, 1);
    const openId = openEntryId(wrapper);

    const star = pane(wrapper).get(".star-toggle");
    expect(star.attributes("aria-label")).toBe("Star entry");

    await star.trigger("click");
    await flush();

    expect(backend.entries.find((e) => e.id === openId)?.is_starred).toBe(true);
    expect((pane(wrapper).props("entry") as { is_starred: boolean }).is_starred).toBe(true);
    expect(pane(wrapper).get(".star-toggle").attributes("aria-label")).toBe(
      "Unstar entry",
    );
  });

  it("unstars from the pane", async () => {
    const wrapper = await mountApp();
    await openRow(wrapper, 1);
    const openId = openEntryId(wrapper);
    await pane(wrapper).get(".star-toggle").trigger("click");
    await flush();

    await pane(wrapper).get(".star-toggle").trigger("click");
    await flush();

    expect(backend.entries.find((e) => e.id === openId)?.is_starred).toBe(false);
  });

  it("leaves the read state alone when starring", async () => {
    const wrapper = await mountApp();
    await openRow(wrapper, 1);
    expect(pane(wrapper).text()).toContain("Mark unread");

    await pane(wrapper).get(".star-toggle").trigger("click");
    await flush();

    // Still read, so still offering to undo that.
    expect(pane(wrapper).text()).toContain("Mark unread");
  });

  it("still stars an entry whose row is no longer loaded", async () => {
    const wrapper = await mountApp();
    // Opening marks it read, so switching the list to unread drops its row
    // while the article stays open.
    await openRow(wrapper, 1);
    const openId = openEntryId(wrapper);
    await wrapper.findAll(".view-tabs button")[1].trigger("click");
    await flush();

    expect(list(wrapper).props("entries").some((e) => e.id === openId)).toBe(false);

    await pane(wrapper).get(".star-toggle").trigger("click");
    await flush();

    expect(backend.entries.find((e) => e.id === openId)?.is_starred).toBe(true);
    expect((pane(wrapper).props("entry") as { is_starred: boolean }).is_starred).toBe(true);
  });

  it("opens the next entry on a leftward swipe", async () => {
    const wrapper = await mountApp();
    await openRow(wrapper, 1);
    const before = openEntryId(wrapper);
    const nextId = list(wrapper).props("entries")[2].id;

    await swipe(wrapper, -200);

    expect(openEntryId(wrapper)).toBe(nextId);
    expect(nextId).not.toBe(before);
    // Opening it marked it read, exactly as tapping it would.
    expect(backend.entries.find((e) => e.id === nextId)?.is_read).toBe(true);
  });

  it("opens the previous entry on a rightward swipe", async () => {
    const wrapper = await mountApp();
    await openRow(wrapper, 2);
    const previousId = list(wrapper).props("entries")[1].id;

    await swipe(wrapper, 200);

    expect(openEntryId(wrapper)).toBe(previousId);
  });

  it("continues into the next batch when the swipe walks off the page", async () => {
    backend = new FakeBackend({ entries: 120, feeds: 1 });
    harness.backend = backend;
    const wrapper = mount(App, { attachTo: document.body });
    await flush();

    const rows = wrapper.findAll(".entry");
    const lastLoaded = rows.length - 1;
    await openRow(wrapper, lastLoaded);
    expect(rows.length).toBe(50);

    await swipe(wrapper, -200);

    expect(backend.countCalls("list_entries")).toBeGreaterThan(1);
    expect(openEntryId(wrapper)).toBe(list(wrapper).props("entries")[50].id);
  });

  it("keeps the open entry when there is nothing next", async () => {
    const wrapper = await mountApp();
    const rows = wrapper.findAll(".entry");
    await openRow(wrapper, rows.length - 1);
    const openId = openEntryId(wrapper);

    await swipe(wrapper, -200);

    expect(openEntryId(wrapper)).toBe(openId);
  });

  it("scrolls rather than switching on a vertical drag", async () => {
    const wrapper = await mountApp();
    await openRow(wrapper, 1);
    const before = openEntryId(wrapper);

    press(wrapper.get(".content").element, "pointerdown", 300, 300);
    press(window, "pointermove", 304, 420);
    press(window, "pointerup", 304, 420);
    await settleBurst();

    expect(openEntryId(wrapper)).toBe(before);
  });

  it("offers stepped navigation and disables it at each end", async () => {
    const wrapper = await mountApp();

    // The newest entry: nothing before it.
    await openRow(wrapper, 0);
    expect(
      pane(wrapper).get('[aria-label="Previous entry"]').attributes("disabled"),
    ).toBeDefined();
    expect(
      pane(wrapper).get('[aria-label="Next entry"]').attributes("disabled"),
    ).toBeUndefined();

    // The oldest loaded row, with nothing further to load.
    await openRow(wrapper, wrapper.findAll(".entry").length - 1);
    expect(
      pane(wrapper).get('[aria-label="Next entry"]').attributes("disabled"),
    ).toBeDefined();
    expect(
      pane(wrapper).get('[aria-label="Previous entry"]').attributes("disabled"),
    ).toBeUndefined();
  });

  it("moves to the next entry from the step control", async () => {
    const wrapper = await mountApp();
    await openRow(wrapper, 1);
    const nextId = list(wrapper).props("entries")[2].id;

    await pane(wrapper).get('[aria-label="Next entry"]').trigger("click");
    await settleBurst();

    expect(openEntryId(wrapper)).toBe(nextId);
  });

  it("does not offer the steps on a pointer layout", async () => {
    viewport.set({ narrow: false, coarseNarrow: false, coarse: false });
    const wrapper = await mountApp();
    await openRow(wrapper, 1);

    expect(pane(wrapper).find(".pane-steps").exists()).toBe(false);
  });
});

describe("returning from the reader", () => {
  it("brings the entry just read into view", async () => {
    const wrapper = await mountApp();
    const scrolled: string[] = [];
    // jsdom reports a zero rect and has no scrolling, so record the request.
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = function (this: Element) {
      const id = this.getAttribute("data-entry-id");
      if (id !== null) scrolled.push(id);
    };

    try {
      await wrapper.findAll(".entry")[1].trigger("click");
      await flush();
      const readId = openEntryId(wrapper);

      pane(wrapper).vm.$emit("close");
      await nextTick();
      await nextTick();

      expect(list(wrapper).isVisible()).toBe(true);
      expect(list(wrapper).props("selectedEntryId")).toBe(readId);
      expect(scrolled).toContain(String(readId));
    } finally {
      Element.prototype.scrollIntoView = original;
    }
  });

  it("reveals the entry last reached by swiping", async () => {
    const wrapper = await mountApp();
    const scrolled: string[] = [];
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = function (this: Element) {
      const id = this.getAttribute("data-entry-id");
      if (id !== null) scrolled.push(id);
    };

    try {
      await wrapper.findAll(".entry")[1].trigger("click");
      await flush();
      const content = wrapper.get(".content");
      press(content.element, "pointerdown", 300, 300);
      press(window, "pointermove", 100, 302);
      press(window, "pointerup", 100, 302);
      await new Promise((resolve) => setTimeout(resolve, 180));
      await flush();

      const swipedTo = openEntryId(wrapper);
      pane(wrapper).vm.$emit("close");
      await nextTick();
      await nextTick();

      expect(list(wrapper).props("selectedEntryId")).toBe(swipedTo);
      expect(scrolled).toContain(String(swipedTo));
    } finally {
      Element.prototype.scrollIntoView = original;
    }
  });
});

/** The arguments of the most recent per-feed refresh. */
function lastRefreshArgs(): Record<string, unknown> | undefined {
  const calls = backend.calls.filter((c) => c.cmd === "refresh_feed");
  return calls[calls.length - 1]?.args;
}

/**
 * A downward drag on the list itself, as touch events.
 *
 * The pull listens to touch rather than pointer events on purpose: inside a
 * scroll port the browser claims the vertical pan and cancels the pointer
 * stream, so a pointer-driven pull can never commit on a device.
 */
function touch(target: EventTarget, type: string, x: number, y: number) {
  const event = new Event(type, { bubbles: true, cancelable: true });
  const point = { clientX: x, clientY: y };
  Object.assign(event, {
    touches: type === "touchend" ? [] : [point],
    changedTouches: [point],
  });
  target.dispatchEvent(event);
  return event;
}

async function pullList(wrapper: VueWrapper, dy: number) {
  const listEl = wrapper.get(".entries").element;
  touch(listEl, "touchstart", 180, 120);
  touch(listEl, "touchmove", 180, 120 + dy / 2);
  touch(listEl, "touchmove", 180, 120 + dy);
  touch(listEl, "touchend", 180, 120 + dy);
  await flush();
}

describe("pulling the list to refresh", () => {
  it("refreshes the feed the list is showing", async () => {
    const wrapper = await mountApp();
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 1 });
    await flush();
    const before = backend.countCalls("refresh_feed");

    await pullList(wrapper, 200);

    expect(backend.countCalls("refresh_feed")).toBe(before + 1);
    expect(lastRefreshArgs()).toEqual({ feedId: 1 });
  });

  it("refreshes every feed when the list shows all entries", async () => {
    const wrapper = await mountApp();
    const before = backend.countCalls("refresh_feed");

    await pullList(wrapper, 200);

    expect(backend.countCalls("refresh_feed")).toBe(before + 1);
    expect(lastRefreshArgs()).toEqual({ feedId: 1 });
  });

  it("refreshes nothing when the pull is too short", async () => {
    const wrapper = await mountApp();
    const before = backend.countCalls("refresh_feed");

    await pullList(wrapper, 40);

    expect(backend.countCalls("refresh_feed")).toBe(before);
  });

  it("scrolls instead of refreshing when the list is not at its top", async () => {
    const wrapper = await mountApp();
    const listEl = wrapper.get(".entries").element;
    Object.defineProperty(listEl, "scrollTop", { value: 400, configurable: true });
    const before = backend.countCalls("refresh_feed");

    await pullList(wrapper, 200);

    expect(backend.countCalls("refresh_feed")).toBe(before);
  });

  it("reports the pull in the panel rather than refreshing silently", async () => {
    const wrapper = await mountApp();
    backend.defaultRefresh = { new_entries: 2 };

    await pullList(wrapper, 200);

    expect(wrapper.find(".refresh-panel").exists()).toBe(true);
  });

  it("does not start a second run while one is in flight", async () => {
    const wrapper = await mountApp();
    const gate = backend.holdNext("refresh_feed");

    await pullList(wrapper, 200);
    const inFlight = backend.countCalls("refresh_feed");
    await pullList(wrapper, 200);

    expect(backend.countCalls("refresh_feed")).toBe(inFlight);
    gate.release();
    await flush();
  });
});
