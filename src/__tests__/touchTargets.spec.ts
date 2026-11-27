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

/** A viewport the test controls, standing in for `window.matchMedia`. */
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

function sidebar(wrapper: VueWrapper) {
  return wrapper.findComponent(Sidebar);
}

function pane(wrapper: VueWrapper) {
  return wrapper.findComponent(ReadingPane);
}

function list(wrapper: VueWrapper) {
  return wrapper.findComponent(EntryList);
}

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
  media = new FakeMediaQueryList(true);
  vi.stubGlobal("matchMedia", () => media);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete document.documentElement.dataset.layout;
});

describe("touch layout controls", () => {
  it("gives every entry-row control an accessible name and a touch target", async () => {
    const wrapper = await mountApp();

    const star = wrapper.get(".entry .star");
    expect(star.attributes("aria-label")).toBe("Star entry");
    expect(star.classes()).toContain("touch-target");

    const markAbove = wrapper.get(".entry .mark-above");
    // Count-correct wording: the second row has exactly one unread row above it.
    expect(markAbove.attributes("aria-label")).toBe("Mark 1 entry above as read");
    expect(markAbove.classes()).toContain("touch-target");
  });

  it("gives the header controls accessible names", async () => {
    const wrapper = await mountApp();

    expect(wrapper.get(".nav-toggle").attributes("aria-label")).toBe(
      "Open folders and feeds",
    );
    expect(wrapper.get(".refresh-all").attributes("aria-label")).toBe(
      "Refresh all feeds",
    );
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
    media.matches = false;
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
    media.matches = false;
    const wrapper = await mountApp();
    await wrapper.get(".entry").trigger("click");
    await flush();
    await nextTick();

    expect(pane(wrapper).find(".back").exists()).toBe(false);
  });
});
