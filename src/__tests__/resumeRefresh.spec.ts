import { enableAutoUnmount, mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App.vue";
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
}

let media: FakeMediaQueryList;
let backend: FakeBackend;
let visibility: DocumentVisibilityState;

/** Simulate the app going to the background and coming back after `elapsedMs`. */
function backgroundFor(elapsedMs: number) {
  const started = Date.now();
  visibility = "hidden";
  document.dispatchEvent(new Event("visibilitychange"));
  vi.spyOn(Date, "now").mockReturnValue(started + elapsedMs);
  visibility = "visible";
  document.dispatchEvent(new Event("visibilitychange"));
}

async function mountApp(): Promise<VueWrapper> {
  backend = new FakeBackend({ entries: 5, feeds: 2 });
  harness.backend = backend;
  const wrapper = mount(App, { attachTo: document.body });
  await flush();
  return wrapper;
}

function calls(cmd: string) {
  return backend.calls.filter((c) => c.cmd === cmd);
}

// Every mounted app registers document-level listeners; leaving one alive
// would let a previous test's app answer this test's visibility events.
enableAutoUnmount(afterEach);

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
  media = new FakeMediaQueryList(true);
  vi.stubGlobal("matchMedia", () => media);
  visibility = "visible";
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    get: () => visibility,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete document.documentElement.dataset.layout;
});

describe("resume refresh", () => {
  it("refreshes the current selection when the app returns from the background", async () => {
    await mountApp();
    const before = calls("refresh_all").length;

    backgroundFor(5 * 60_000);
    await flush();

    expect(calls("refresh_all")).toHaveLength(before + 1);
  });

  it("reloads the list so new entries are shown", async () => {
    const wrapper = await mountApp();
    const before = calls("list_entries").length;

    backgroundFor(5 * 60_000);
    await flush();

    expect(calls("list_entries").length).toBeGreaterThan(before);
    const sidebar = wrapper.findComponent(Sidebar);
    expect(sidebar.props("busy")).toBe(false);
  });

  it("refreshes only the selected feed", async () => {
    const wrapper = await mountApp();
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();
    const before = calls("refresh_feed").length;

    backgroundFor(5 * 60_000);
    await flush();

    const feedCalls = calls("refresh_feed");
    expect(feedCalls).toHaveLength(before + 1);
    expect(feedCalls[feedCalls.length - 1].args).toEqual({ feedId: 2 });
  });

  it("refreshes a selected folder rather than the whole library", async () => {
    const wrapper = await mountApp();
    backend.folders.push({ id: 1, name: "Tech", unread_count: 0, feeds: [] });
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "folder", id: 1 });
    await flush();
    const before = calls("refresh_folder").length;

    backgroundFor(5 * 60_000);
    await flush();

    expect(calls("refresh_folder")).toHaveLength(before + 1);
    expect(calls("refresh_all")).toHaveLength(0);
  });

  it("does not refresh a quick interruption", async () => {
    await mountApp();
    const before = calls("refresh_all").length;

    backgroundFor(2_000);
    await flush();

    expect(calls("refresh_all")).toHaveLength(before);
  });

  it("does not refresh on the wide layout", async () => {
    media.matches = false;
    await mountApp();
    const before = calls("refresh_all").length;

    backgroundFor(10 * 60_000);
    await flush();

    expect(calls("refresh_all")).toHaveLength(before);
  });

  it("reports a failed refresh without blocking the list", async () => {
    const wrapper = await mountApp();
    vi.spyOn(backend, "invoke").mockImplementation(async (cmd: string) => {
      if (cmd === "refresh_all") throw new Error("offline");
      return null;
    });

    backgroundFor(5 * 60_000);
    await flush();

    expect(wrapper.get(".global-error").text()).toContain("offline");
    expect(wrapper.findComponent(Sidebar).props("busy")).toBe(false);
  });

  it("stops listening once the app is gone", async () => {
    const wrapper = await mountApp();
    wrapper.unmount();
    const before = calls("refresh_all").length;

    backgroundFor(10 * 60_000);
    await flush();

    expect(calls("refresh_all")).toHaveLength(before);
  });
});
