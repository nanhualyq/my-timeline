import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import EntryList from "../components/EntryList.vue";
import type { EntryListItem } from "../api";
import {
  FakeIntersectionObserver,
  stubScrollGeometry,
} from "./fakeBackend";

function makeRow(id: number): EntryListItem {
  const stamp = `2026-01-01T00:${String(id).padStart(2, "0")}:00+00:00`;
  return {
    id,
    feed_id: 1,
    title: `Entry ${id}`,
    link: null,
    published_at: stamp,
    sort_key: stamp,
    is_read: false,
    is_starred: false,
  };
}

function render(overrides: Partial<Record<string, unknown>> = {}): VueWrapper {
  return mount(EntryList, {
    props: {
      entries: [makeRow(2), makeRow(1)],
      selectedEntryId: null,
      view: "all",
      busy: false,
      unreadFilterAvailable: true,
      viewedCount: 2,
      total: 120,
      hasMore: true,
      loadingMore: false,
      ...overrides,
    },
  });
}

function emittedLoadMore(wrapper: VueWrapper): number {
  return (wrapper.emitted("loadMore") ?? []).length;
}

beforeEach(() => {
  FakeIntersectionObserver.reset();
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  stubScrollGeometry(false);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("scroll sentinel", () => {
  it("is observed against the scrolling list, slightly before the end", () => {
    render();
    const observer = FakeIntersectionObserver.last();
    expect(observer.options?.root).toBeTruthy();
    expect(observer.options?.rootMargin).toBe("150px 0px");
    expect(observer.target?.classList.contains("sentinel")).toBe(true);
  });

  it("asks for another batch when it comes into view", () => {
    const wrapper = render();
    FakeIntersectionObserver.last().trigger(true);
    expect(emittedLoadMore(wrapper)).toBe(1);
  });

  it("stays quiet while it is out of view", () => {
    const wrapper = render();
    FakeIntersectionObserver.last().trigger(false);
    expect(emittedLoadMore(wrapper)).toBe(0);
  });

  it("does not ask once there is nothing more to load", () => {
    const wrapper = render({ hasMore: false });
    FakeIntersectionObserver.last().trigger(true);
    expect(emittedLoadMore(wrapper)).toBe(0);
  });

  it("asks again after a batch lands if it is still on screen", async () => {
    stubScrollGeometry(true);
    const wrapper = render();
    await wrapper.setProps({ loadingMore: true });
    await wrapper.setProps({ loadingMore: false });
    expect(emittedLoadMore(wrapper)).toBe(1);
  });

  it("does not ask again when the finished batch scrolled it away", async () => {
    stubScrollGeometry(false);
    const wrapper = render();
    await wrapper.setProps({ loadingMore: true });
    await wrapper.setProps({ loadingMore: false });
    expect(emittedLoadMore(wrapper)).toBe(0);
  });
});

describe("footer and states", () => {
  it("shows loaded rows against the filtered total", () => {
    const wrapper = render({ viewedCount: 150, total: 12438 });
    expect(wrapper.find(".progress").text()).toBe("150 / 12438 entries viewed");
  });

  it("marks the end of the list", async () => {
    const wrapper = render({ hasMore: false });
    expect(wrapper.find(".sentinel").text()).toBe("End of list");

    await wrapper.setProps({ hasMore: true });
    expect(wrapper.find(".sentinel").text()).toBe("Scroll for more");

    await wrapper.setProps({ loadingMore: true });
    expect(wrapper.find(".sentinel").text()).toBe("Loading…");
  });

  it("hides the sentinel while there is nothing to scroll", () => {
    const wrapper = render({ entries: [], viewedCount: 0, hasMore: false });
    expect(wrapper.find(".empty").text()).toBe("No entries.");
    expect(wrapper.find(".sentinel").classes()).toContain("hidden");
  });

  it("has no previous/next controls or arrival banner any more", () => {
    const wrapper = render();
    expect(wrapper.find(".pager").exists()).toBe(false);
    expect(wrapper.find(".new-arrivals").exists()).toBe(false);
    expect(wrapper.findAll(".entry")).toHaveLength(2);
  });
});
