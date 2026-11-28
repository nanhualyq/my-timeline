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

async function mountApp(
  options: { feeds?: number; lastFetchedAt?: string | null } = {},
): Promise<VueWrapper> {
  backend = new FakeBackend({ entries: 5, feeds: options.feeds ?? 2 });
  if (options.lastFetchedAt !== undefined) {
    for (const feed of backend.feeds) {
      feed.last_fetched_at = options.lastFetchedAt;
    }
  }
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
  document.querySelectorAll(".notice-stack").forEach((node) => node.remove());
});

describe("coming back to the app", () => {
  it("refreshes what the reader is looking at when its content went stale", async () => {
    await mountApp({ feeds: 2, lastFetchedAt: null });
    const before = calls("refresh_feed").length;

    backgroundFor(5 * 60_000);
    await flush();

    expect(calls("refresh_feed").length).toBe(before + 2);
  });

  it("leaves a feed that is still fresh alone", async () => {
    const justNow = new Date().toISOString();
    await mountApp({ feeds: 2, lastFetchedAt: justNow });
    const before = calls("refresh_feed").length;

    backgroundFor(5 * 60_000);
    await flush();

    expect(calls("refresh_feed").length).toBe(before);
  });

  it("reloads the list so new entries are shown", async () => {
    const wrapper = await mountApp({ lastFetchedAt: null });
    const before = calls("list_entries").length;

    backgroundFor(5 * 60_000);
    await flush();

    expect(calls("list_entries").length).toBeGreaterThan(before);
    expect(wrapper.findComponent(Sidebar).props("busy")).toBe(false);
  });

  it("refreshes only the selected feed", async () => {
    const wrapper = await mountApp({ feeds: 2, lastFetchedAt: null });
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "feed", id: 2 });
    await flush();
    const before = calls("refresh_feed").length;

    backgroundFor(5 * 60_000);
    await flush();

    const fetched = calls("refresh_feed").slice(before);
    expect(fetched).toHaveLength(1);
    expect(fetched[0].args).toEqual({ feedId: 2 });
  });

  it("refreshes a selected folder's feeds rather than the whole library", async () => {
    const wrapper = await mountApp({ feeds: 3, lastFetchedAt: null });
    backend.feeds[1].folder_id = 1;
    backend.folders.push({ id: 1, name: "Tech", unread_count: 0, feeds: [] });
    wrapper.findComponent(Sidebar).vm.$emit("select", { kind: "folder", id: 1 });
    await flush();
    const before = calls("refresh_feed").length;

    backgroundFor(5 * 60_000);
    await flush();

    const fetched = calls("refresh_feed").slice(before);
    expect(fetched).toHaveLength(1);
    expect(fetched[0].args).toEqual({ feedId: 2 });
  });

  it("does not refresh a quick interruption", async () => {
    await mountApp({ lastFetchedAt: null });
    const before = calls("refresh_feed").length;

    backgroundFor(2_000);
    await flush();

    expect(calls("refresh_feed").length).toBe(before);
  });

  it("does not refresh on the wide layout", async () => {
    media.matches = false;
    await mountApp({ lastFetchedAt: null });
    const before = calls("refresh_feed").length;

    backgroundFor(10 * 60_000);
    await flush();

    expect(calls("refresh_feed").length).toBe(before);
  });

  it("reports a failing feed in the panel instead of blocking the list", async () => {
    const wrapper = await mountApp({ feeds: 1, lastFetchedAt: null });
    backend.defaultRefresh = { error: "HTTP 500 Internal Server Error", http_status: 500 };

    backgroundFor(5 * 60_000);
    await flush();

    const panel = wrapper.find(".refresh-panel");
    expect(panel.exists()).toBe(true);
    expect(panel.text()).toContain("failed");
    expect(wrapper.findComponent(Sidebar).props("busy")).toBe(false);
  });

  it("stops listening once the app is gone", async () => {
    const wrapper = await mountApp({ lastFetchedAt: null });
    wrapper.unmount();
    const before = calls("refresh_feed").length;

    backgroundFor(10 * 60_000);
    await flush();

    expect(calls("refresh_feed").length).toBe(before);
  });
});

describe("the schedule", () => {
  /** Mount with the schedule on and a library whose feeds are all stale. */
  async function mountScheduled(): Promise<void> {
    backend = new FakeBackend({ entries: 5, feeds: 2 });
    backend.refreshSettings = { enabled: true, interval_minutes: 15 };
    for (const feed of backend.feeds) feed.last_fetched_at = null;
    harness.backend = backend;
    mount(App, { attachTo: document.body });
    await vi.advanceTimersByTimeAsync(0);
  }

  it("refreshes stale feeds on its own when it is on", async () => {
    vi.useFakeTimers();
    try {
      await mountScheduled();
      const before = calls("refresh_feed").length;
      expect(before, "the first tick is the schedule's").toBe(0);

      await vi.advanceTimersByTimeAsync(60_000);

      expect(calls("refresh_feed").length).toBe(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("does nothing while it is off", async () => {
    vi.useFakeTimers();
    try {
      backend = new FakeBackend({ entries: 5, feeds: 1 });
      backend.feeds[0].last_fetched_at = null;
      harness.backend = backend;
      mount(App, { attachTo: document.body });
      await vi.advanceTimersByTimeAsync(0);

      await vi.advanceTimersByTimeAsync(5 * 60_000);

      expect(calls("refresh_feed")).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not fetch while the app is hidden", async () => {
    vi.useFakeTimers();
    try {
      await mountScheduled();
      visibility = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));

      await vi.advanceTimersByTimeAsync(10 * 60_000);

      expect(calls("refresh_feed")).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it("catches up when the app becomes visible again", async () => {
    vi.useFakeTimers();
    try {
      backend = new FakeBackend({ entries: 5, feeds: 1 });
      backend.refreshSettings = { enabled: true, interval_minutes: 15 };
      backend.feeds[0].last_fetched_at = null;
      harness.backend = backend;
      visibility = "hidden";
      mount(App, { attachTo: document.body });
      await vi.advanceTimersByTimeAsync(60_000);
      expect(calls("refresh_feed"), "hidden means no fetching").toHaveLength(0);

      visibility = "visible";
      document.dispatchEvent(new Event("visibilitychange"));
      await vi.advanceTimersByTimeAsync(0);

      expect(calls("refresh_feed")).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });
});
