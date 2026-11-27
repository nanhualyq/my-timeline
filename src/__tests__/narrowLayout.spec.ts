import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import App from "../App.vue";
import EntryList from "../components/EntryList.vue";
import ReadingPane from "../components/ReadingPane.vue";
import Sidebar from "../components/Sidebar.vue";
import {
  FakeBackend,
  FakeIntersectionObserver,
  flush,
  stubScrollGeometry,
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

/** A viewport the test can resize, standing in for `window.matchMedia`. */
class FakeMediaQueryList {
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
}

let media: FakeMediaQueryList;
/** A phone has no hovering pointer; the touch layout tests assume that. */
let hoverMedia: FakeMediaQueryList;
let backend: FakeBackend;

function build() {
  backend = new FakeBackend({ entries: 12, feeds: 1 });
  harness.backend = backend;
}

async function mountApp(): Promise<VueWrapper> {
  build();
  const wrapper = mount(App, { attachTo: document.body });
  await flush();
  return wrapper;
}

function sidebar(wrapper: VueWrapper) {
  return wrapper.findComponent(Sidebar);
}

function list(wrapper: VueWrapper) {
  return wrapper.findComponent(EntryList);
}

function pane(wrapper: VueWrapper) {
  return wrapper.findComponent(ReadingPane);
}

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
  // Narrow by default in this file: every test here is about the one-pane
  // layout. `resize(false)` crosses the breakpoint.
  media = new FakeMediaQueryList(true);
  hoverMedia = new FakeMediaQueryList(false);
  // One viewport, two questions asked of it: the width breakpoint and whether
  // a pointer can hover.
  vi.stubGlobal("matchMedia", (query: string) =>
    query.includes("hover") ? hoverMedia : media,
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete document.documentElement.dataset.layout;
});

describe("narrow layout", () => {
  it("publishes the layout to the document", async () => {
    await mountApp();
    expect(document.documentElement.dataset.layout).toBe("narrow");
  });

  it("does not offer the keyboard reference on a touch-only layout", async () => {
    // A phone: narrow and no hovering pointer. No visible affordance may imply
    // that shortcuts are how the app is used.
    const wrapper = await mountApp();

    expect(wrapper.text()).not.toContain("Keyboard");
    expect(wrapper.text()).not.toMatch(/press \?/i);

    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "?", bubbles: true }),
    );
    await nextTick();

    expect(document.querySelector(".help-overlay")).toBeNull();
  });

  it("still offers the keyboard reference to a narrow layout with a keyboard", async () => {
    // A phone with a physical keyboard, or a narrow desktop window.
    hoverMedia.matches = true;
    await mountApp();

    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "?", bubbles: true }),
    );
    await nextTick();

    expect(document.querySelector(".help-overlay")).not.toBeNull();
  });

  it("shows one pane: the list, with no drawer and no reading pane", async () => {
    const wrapper = await mountApp();

    expect(list(wrapper).isVisible()).toBe(true);
    expect(sidebar(wrapper).isVisible()).toBe(false);
    expect(pane(wrapper).isVisible()).toBe(false);
    expect(wrapper.find(".drawer-backdrop").exists()).toBe(false);
  });

  it("opens the drawer from the list header and closes it on the backdrop", async () => {
    const wrapper = await mountApp();

    await wrapper.get(".nav-toggle").trigger("click");
    expect(sidebar(wrapper).isVisible()).toBe(true);
    expect(wrapper.find(".drawer-backdrop").exists()).toBe(true);

    await wrapper.get(".drawer-backdrop").trigger("click");
    expect(sidebar(wrapper).isVisible()).toBe(false);
    expect(wrapper.find(".drawer-backdrop").exists()).toBe(false);
  });

  it("closes the drawer and loads the new selection when a feed is chosen", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".nav-toggle").trigger("click");
    const before = backend.calls.filter((c) => c.cmd === "list_entries").length;

    sidebar(wrapper).vm.$emit("select", { kind: "feed", id: 1 });
    await flush();

    expect(sidebar(wrapper).isVisible()).toBe(false);
    expect(list(wrapper).isVisible()).toBe(true);
    expect(
      backend.calls.filter((c) => c.cmd === "list_entries").length,
    ).toBeGreaterThan(before);
  });

  it("opens the reading screen on a row tap and returns to the list", async () => {
    const wrapper = await mountApp();
    const row = wrapper.get(".entry");

    await row.trigger("click");
    await flush();

    expect(pane(wrapper).isVisible()).toBe(true);
    expect(list(wrapper).isVisible()).toBe(false);
    expect(pane(wrapper).props("narrow")).toBe(true);

    pane(wrapper).vm.$emit("close");
    await nextTick();

    expect(pane(wrapper).isVisible()).toBe(false);
    expect(list(wrapper).isVisible()).toBe(true);
    // The row the reader came from is still the selected one.
    expect(list(wrapper).props("selectedEntryId")).not.toBeNull();
  });

  it("offers the reading screen its own way back", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();

    const back = pane(wrapper).get(".back");
    expect(back.attributes("aria-label")).toBe("Back to entry list");

    await back.trigger("click");
    await nextTick();
    expect(list(wrapper).isVisible()).toBe(true);
  });

  it("passes the layout down so rows and header can adapt", async () => {
    const wrapper = await mountApp();

    expect(list(wrapper).props("narrow")).toBe(true);
    expect(wrapper.get(".list-header").classes()).toContain("narrow-header");
    expect(wrapper.get(".entry").classes()).toContain("narrow-row");
    // The list header keeps both actions reachable at phone width.
    expect(wrapper.find(".refresh-all").exists()).toBe(true);
  });

  it("keeps an open entry when the viewport becomes wide again", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();
    expect(pane(wrapper).isVisible()).toBe(true);

    media.resize(false);
    await nextTick();

    expect(document.documentElement.dataset.layout).toBe("wide");
    // All three columns are on screen at once, with the same entry open.
    expect(sidebar(wrapper).isVisible()).toBe(true);
    expect(list(wrapper).isVisible()).toBe(true);
    expect(pane(wrapper).isVisible()).toBe(true);
  });

  it("closes the drawer when the viewport becomes wide", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".nav-toggle").trigger("click");
    expect(wrapper.find(".drawer-backdrop").exists()).toBe(true);

    media.resize(false);
    await nextTick();

    expect(wrapper.find(".drawer-backdrop").exists()).toBe(false);
    expect(sidebar(wrapper).isVisible()).toBe(true);
  });

  it("leaves the reader closed when the viewport becomes narrow with no entry open", async () => {
    media.matches = false;
    const wrapper = await mountApp();
    expect(pane(wrapper).isVisible()).toBe(true);

    media.resize(true);
    await nextTick();

    expect(pane(wrapper).isVisible()).toBe(false);
    expect(list(wrapper).isVisible()).toBe(true);
  });
});

/** The system back action reaches the WebView as a history pop. */
function systemBack() {
  window.dispatchEvent(new PopStateEvent("popstate"));
}

describe("narrow layout back navigation", () => {
  it("closes the drawer", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".nav-toggle").trigger("click");
    expect(sidebar(wrapper).isVisible()).toBe(true);

    systemBack();
    await nextTick();

    expect(sidebar(wrapper).isVisible()).toBe(false);
    expect(list(wrapper).isVisible()).toBe(true);
  });

  it("closes the reading screen before the drawer", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();
    expect(pane(wrapper).isVisible()).toBe(true);

    systemBack();
    await nextTick();

    expect(pane(wrapper).isVisible()).toBe(false);
    expect(list(wrapper).isVisible()).toBe(true);
  });

  it("closes a dialog before anything under it", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();

    sidebar(wrapper).vm.$emit("openSettings");
    await nextTick();
    // The dialog teleports to <body>, so it is outside the wrapper's tree.
    expect(document.querySelector(".dialog-overlay")).not.toBeNull();

    systemBack();
    await nextTick();

    expect(document.querySelector(".dialog-overlay")).toBeNull();
    // The reading screen underneath was not touched.
    expect(pane(wrapper).isVisible()).toBe(true);
  });

  it("pushes one history entry per open layer", async () => {
    const wrapper = await mountApp();
    const before = window.history.length;

    await wrapper.get(".nav-toggle").trigger("click");
    await nextTick();

    expect(window.history.length).toBe(before + 1);
  });
});
