import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import App from "../App.vue";
import EntryList from "../components/EntryList.vue";
import UndoBar from "../components/UndoBar.vue";
import {
  FakeBackend,
  FakeIntersectionObserver,
  flush,
  installViewport,
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

let backend: FakeBackend;

/** A desktop window: the batch actions are visible controls here, which is the
 *  simplest way to reach them from a test. */
async function mountApp(entries = 12): Promise<VueWrapper> {
  backend = new FakeBackend({ entries, feeds: 1 });
  harness.backend = backend;
  const wrapper = mount(App, { attachTo: document.body });
  await flush();
  return wrapper;
}

/** `flush` waits on timers, which a test that fakes them cannot do; the fake
 *  backend resolves immediately, so draining microtasks is enough. */
async function settle(times = 40): Promise<void> {
  for (let i = 0; i < times; i += 1) await Promise.resolve();
}

async function mountAppWithFakeTimers(entries = 12): Promise<VueWrapper> {
  backend = new FakeBackend({ entries, feeds: 1 });
  harness.backend = backend;
  const wrapper = mount(App, { attachTo: document.body });
  await settle();
  return wrapper;
}

function list(wrapper: VueWrapper) {
  return wrapper.findComponent(EntryList);
}

function undoBar(wrapper: VueWrapper) {
  return wrapper.findComponent(UndoBar);
}

/** Unread entries by id, straight from the fake library. */
function unreadIds(): number[] {
  return backend.entries.filter((e) => !e.is_read).map((e) => e.id);
}

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
  installViewport({});
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete document.documentElement.dataset.layout;
  delete document.documentElement.dataset.input;
});

describe("undo after a batch mark", () => {
  it("offers undo after the header action", async () => {
    const wrapper = await mountApp();
    const before = unreadIds().length;

    await wrapper.get(".mark-rendered").trigger("click");
    await flush();

    expect(undoBar(wrapper).exists()).toBe(true);
    expect(undoBar(wrapper).props("count")).toBe(before);
    expect(wrapper.get(".undo-text").text()).toContain(`${before} entries`);
  });

  it("offers undo after the row action", async () => {
    const wrapper = await mountApp();

    list(wrapper).vm.$emit("markAbove", 3);
    await flush();

    expect(undoBar(wrapper).exists()).toBe(true);
    expect(undoBar(wrapper).props("count")).toBeGreaterThan(0);
  });

  it("restores exactly the entries the batch marked", async () => {
    const wrapper = await mountApp();
    const before = unreadIds();

    await wrapper.get(".mark-rendered").trigger("click");
    await flush();
    expect(unreadIds().length).toBeLessThan(before.length);

    await wrapper.get(".undo-action").trigger("click");
    await flush();

    expect(unreadIds().sort((a, b) => a - b)).toEqual(before.sort((a, b) => a - b));
    expect(undoBar(wrapper).exists()).toBe(false);
  });

  it("brings the restored rows back into an unread list", async () => {
    const wrapper = await mountApp();
    const before = list(wrapper).props("entries").length;

    await wrapper.get(".mark-rendered").trigger("click");
    await flush();

    await wrapper.get(".undo-action").trigger("click");
    await flush();

    // The marked rows left the page; undo refills it rather than leaving the
    // reader looking at a list that does not show what came back.
    expect(list(wrapper).props("entries").length).toBe(before);
    expect(list(wrapper).props("entries").every((e) => !e.is_read)).toBe(true);
  });

  it("leaves an entry the batch did not mark alone", async () => {
    const wrapper = await mountApp();
    // A row the batch will not touch: seeded read, so it is in neither the
    // rendered unread rows nor the batch.
    const bystander = backend.entries.find((e) => e.is_read);
    expect(bystander).toBeDefined();

    await wrapper.get(".mark-rendered").trigger("click");
    await flush();
    // Something else marks and unmarks entries while the offer is up.
    if (bystander) bystander.is_read = false;

    await wrapper.get(".undo-action").trigger("click");
    await flush();

    expect(bystander?.is_read).toBe(false);
  });

  it("withdraws the offer when its window passes", async () => {
    vi.useFakeTimers();
    const wrapper = await mountAppWithFakeTimers();

    await wrapper.get(".mark-rendered").trigger("click");
    await settle();
    expect(undoBar(wrapper).exists()).toBe(true);

    vi.advanceTimersByTime(3100);
    await settle();
    await nextTick();

    expect(undoBar(wrapper).exists()).toBe(false);
  });

  it("restores only the newer batch when a second one replaces it", async () => {
    const wrapper = await mountApp(120);

    const start = unreadIds();
    await wrapper.get(".mark-rendered").trigger("click");
    await flush();
    const afterFirst = unreadIds();
    const firstMarked = start.filter((id) => !afterFirst.includes(id));

    await wrapper.get(".mark-rendered").trigger("click");
    await flush();
    const afterSecond = unreadIds();
    const secondMarked = afterFirst.filter((id) => !afterSecond.includes(id));

    expect(firstMarked.length).toBeGreaterThan(0);
    expect(secondMarked.length).toBeGreaterThan(0);

    await wrapper.get(".undo-action").trigger("click");
    await flush();

    // Only the second batch came back; the first batch is still read.
    expect(unreadIds().sort((a, b) => a - b)).toEqual(secondMarked.sort((a, b) => a - b));
    for (const id of firstMarked) expect(unreadIds()).not.toContain(id);
  });

  it("does not offer undo when the action reached nothing", async () => {
    const wrapper = await mountApp();

    // The first rendered row has no unread row above it, so the row action
    // marks nothing and must not leave an undo offer behind.
    list(wrapper).vm.$emit("markAbove", 0);
    await flush();

    expect(undoBar(wrapper).exists()).toBe(false);
  });
});
