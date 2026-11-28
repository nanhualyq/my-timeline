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

/** The viewport every test in this file starts from: a phone. */
let viewport: InstalledViewport;
let backend: FakeBackend;

async function mountApp(): Promise<VueWrapper> {
  backend = new FakeBackend({ entries: 12, feeds: 1 });
  backend.folders.push({
    id: 1,
    name: "Tech",
    unread_count: 3,
    feeds: [],
  });
  harness.backend = backend;
  const wrapper = mount(App, { attachTo: document.body });
  await flush();
  return wrapper;
}

/** A desktop window: wide, three columns, a pointer. */
function desktop() {
  viewport.set({ narrow: false, coarseNarrow: false, coarse: false });
}

/** A tablet in landscape: three columns operated by a finger. */
function tablet() {
  viewport.set({ narrow: false, coarseNarrow: false, coarse: true });
}

function sidebar(wrapper: VueWrapper) {
  return wrapper.findComponent(Sidebar);
}

function pane(wrapper: VueWrapper) {
  return wrapper.findComponent(ReadingPane);
}

function list(wrapper: VueWrapper) {
  return wrapper.findComponent(EntryList);
}

function row_panels(wrapper: VueWrapper) {
  return wrapper.findAll(".swipe-panel").length;
}

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
  viewport = installViewport({ narrow: true, coarseNarrow: true, coarse: true });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete document.documentElement.dataset.layout;
  delete document.documentElement.dataset.input;
});

describe("touch layout controls", () => {
  it("gives a touch row no controls of its own", async () => {
    const wrapper = await mountApp();

    // The row's actions are the drags and the long-press menu, so nothing sits
    // on the row spending width until a finger asks for it.
    expect(wrapper.find(".entry .star").exists()).toBe(false);
    expect(wrapper.find(".entry .mark-above").exists()).toBe(false);
  });

  it("reveals the drag actions behind a touch row", async () => {
    const wrapper = await mountApp();
    const [newest, second] = wrapper.findAll(".entry");

    // The newest row has nothing above it, so its rightward drag would do
    // nothing and says so.
    expect(newest.find(".swipe-panel.leading").text()).toBe("Nothing unread above");
    expect(second.find(".swipe-panel.leading").text()).toBe(
      "Mark 1 entry above read",
    );
    expect(second.find(".swipe-panel.trailing").text()).toBe("Star");
  });

  it("keeps the row's controls, with their names, on a pointer layout", async () => {
    desktop();
    const wrapper = await mountApp();

    const star = wrapper.get(".entry .star");
    expect(star.attributes("aria-label")).toBe("Star entry");

    const markAbove = wrapper.get(".entry .mark-above");
    // Count-correct wording: the second row has exactly one unread row above it.
    expect(markAbove.attributes("aria-label")).toBe("Mark 1 entry above as read");
    expect(row_panels(wrapper)).toBe(0);
  });

  it("gives the header controls accessible names", async () => {
    const wrapper = await mountApp();

    expect(wrapper.get(".nav-toggle").attributes("aria-label")).toBe(
      "Open folders and feeds",
    );
    // Refreshing on a phone is the pull gesture, so the header keeps only the
    // drawer control — named, like every other control.
    expect(wrapper.find(".refresh-all").exists()).toBe(false);
  });

  it("sizes the drawer's controls for a fingertip", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".nav-toggle").trigger("click");

    const caret = wrapper.get(".folder-toggle");
    expect(caret.classes()).toContain("touch-target");
    expect(caret.attributes("aria-label")).toBe("Expand folder");

    for (const trigger of wrapper.findAll(".row-menu-trigger")) {
      expect(trigger.classes()).toContain("touch-target");
      expect(trigger.attributes("aria-label")).toBeTruthy();
    }

    expect(wrapper.get(".settings-btn").attributes("aria-label")).toBe(
      "Settings",
    );
  });

  it("sizes the reading screen's back control for a fingertip", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();

    const back = pane(wrapper).get(".back");
    expect(back.classes()).toContain("touch-target");
    expect(back.attributes("aria-label")).toBe("Back to entry list");
  });

  it("keeps the wide layout's controls compact", async () => {
    desktop();
    const wrapper = await mountApp();

    // No touch sizing, and no drawer toggle: the desktop layout is unchanged.
    expect(wrapper.get(".entry .star").classes()).not.toContain("touch-target");
    expect(wrapper.find(".nav-toggle").exists()).toBe(false);
    expect(list(wrapper).props("narrow")).toBe(false);
    expect(sidebar(wrapper).props("narrow")).toBe(false);
  });

  it("marks the narrow reading screen so its header can wrap", async () => {
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();

    expect(pane(wrapper).props("narrow")).toBe(true);
    expect(pane(wrapper).get(".pane-header").classes()).toContain(
      "narrow-header",
    );
  });

  it("does not mount the reading screen's back control on the wide layout", async () => {
    desktop();
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();
    await nextTick();

    expect(pane(wrapper).find(".back").exists()).toBe(false);
  });
});

describe("wide touch layout", () => {
  it("publishes three columns and a touch input for a coarse pointer", async () => {
    tablet();
    await mountApp();

    expect(document.documentElement.dataset.layout).toBe("wide");
    expect(document.documentElement.dataset.input).toBe("touch");
  });

  it("tells the list its columns are wide but its input is touch", async () => {
    tablet();
    const wrapper = await mountApp();

    expect(list(wrapper).props("narrow")).toBe(false);
    expect(list(wrapper).props("touch")).toBe(true);
    // The drawer toggle belongs to the one-pane layout, so it stays away.
    expect(wrapper.find(".nav-toggle").exists()).toBe(false);
  });

  it("shows one pane on a landscape phone", async () => {
    // A landscape phone measures about 915 x 412 CSS pixels: wide enough by
    // the column breakpoint, far too short for three usable columns.
    viewport.set({ narrow: false, coarseNarrow: true, coarse: true });
    const wrapper = await mountApp();

    expect(document.documentElement.dataset.layout).toBe("narrow");
    expect(list(wrapper).props("narrow")).toBe(true);
    expect(list(wrapper).props("touch")).toBe(true);
    expect(pane(wrapper).isVisible()).toBe(false);
  });

  it("keeps the desktop window on a pointer", async () => {
    desktop();
    const wrapper = await mountApp();

    expect(document.documentElement.dataset.input).toBe("pointer");
    expect(list(wrapper).props("touch")).toBe(false);
    expect(pane(wrapper).props("touch")).toBe(false);
  });
});
