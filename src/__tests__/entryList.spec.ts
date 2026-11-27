import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import EntryList from "../components/EntryList.vue";
import type { EntryListItem } from "../api";
import {
  FakeIntersectionObserver,
  stubScrollGeometry,
} from "./fakeBackend";

function makeRow(id: number, overrides: Partial<EntryListItem> = {}): EntryListItem {
  const stamp = `2026-01-01T00:${String(id).padStart(2, "0")}:00+00:00`;
  return {
    id,
    feed_id: 1,
    title: `Entry ${id}`,
    link: null,
    excerpt: "",
    image_url: null,
    published_at: stamp,
    sort_key: stamp,
    is_read: false,
    is_starred: false,
    ...overrides,
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
      feedTitles: { 1: "Feed One" },
      showFeedLabel: true,
      batchActionsAvailable: true,
      narrow: false,
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

describe("row detail", () => {
  const NOW = Date.parse("2026-03-10T12:00:00Z");

  function ago(ms: number): string {
    return new Date(NOW - ms).toISOString();
  }

  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders cover, title, excerpt, feed label, relative time, and star", () => {
    const wrapper = render({
      entries: [
        makeRow(1, {
          title: "A title",
          excerpt: "A short excerpt.",
          image_url: "https://img.example/cover.jpg",
          published_at: ago(20 * 60_000),
        }),
      ],
    });

    const row = wrapper.find(".entry");
    expect(row.find(".entry-cover").attributes("src")).toBe(
      "https://img.example/cover.jpg",
    );
    expect(row.find(".entry-cover").attributes("loading")).toBe("lazy");
    expect(row.find(".entry-title").text()).toBe("A title");
    expect(row.find(".entry-excerpt").text()).toBe("A short excerpt.");
    expect(row.find(".entry-feed").text()).toBe("Feed One");
    expect(row.find(".entry-date").text()).toBe("20m");
    expect(row.find(".star").exists()).toBe(true);
  });

  it("puts the full timestamp on the time label", () => {
    const published = ago(3 * 3_600_000);
    const wrapper = render({ entries: [makeRow(1, { published_at: published })] });
    expect(wrapper.find(".entry-date").attributes("title")).toBe(
      new Date(published).toLocaleString(),
    );
  });

  it("omits the cover slot and the excerpt line when absent", () => {
    const wrapper = render({ entries: [makeRow(1)] });
    const row = wrapper.find(".entry");
    expect(row.find(".entry-cover").exists()).toBe(false);
    expect(row.find(".entry-excerpt").exists()).toBe(false);
  });

  it("drops a broken cover without leaving a placeholder", async () => {
    const wrapper = render({
      entries: [makeRow(1, { image_url: "https://img.example/gone.jpg" })],
    });
    expect(wrapper.find(".entry-cover").exists()).toBe(true);

    await wrapper.find(".entry-cover").trigger("error");

    expect(wrapper.find(".entry-cover").exists()).toBe(false);
    expect(wrapper.find(".entry-title").exists()).toBe(true);
  });

  it("hides the feed label in a single feed's view", () => {
    const wrapper = render({ showFeedLabel: false });
    expect(wrapper.find(".entry-feed").exists()).toBe(false);
    expect(wrapper.find(".entry-date").exists()).toBe(true);
  });

  it("has no feed label for an unknown feed", () => {
    const wrapper = render({ entries: [makeRow(1, { feed_id: 99 })] });
    expect(wrapper.find(".entry-feed").exists()).toBe(false);
  });

  it("ages its labels while the list stays open", async () => {
    const wrapper = render({
      entries: [makeRow(1, { published_at: ago(5 * 60_000) })],
    });
    expect(wrapper.find(".entry-date").text()).toBe("5m");

    await vi.advanceTimersByTimeAsync(60_000);

    expect(wrapper.find(".entry-date").text()).toBe("6m");
  });

  it("falls back to the ordering time for an entry with no publication date", () => {
    const wrapper = render({
      entries: [
        makeRow(1, { published_at: null, sort_key: ago(2 * 3_600_000) }),
      ],
    });
    expect(wrapper.find(".entry-date").text()).toBe("2h");
  });
});

describe("batch read actions", () => {
  /** Newest first, as the list renders: three unread and one read in between. */
  function mixedRows(): EntryListItem[] {
    return [
      makeRow(4),
      makeRow(3),
      makeRow(2, { is_read: true }),
      makeRow(1),
    ];
  }

  function markAboveButtons(wrapper: VueWrapper) {
    return wrapper.findAll(".mark-above");
  }

  it("labels the header action with the unread rows on this page", () => {
    const wrapper = render({ entries: mixedRows() });
    expect(wrapper.find(".mark-rendered").text()).toBe("Mark 3 read");
  });

  it("marks the rendered page, not the whole selection", async () => {
    const wrapper = render({ entries: mixedRows(), total: 12_438 });
    await wrapper.find(".mark-rendered").trigger("click");
    expect(wrapper.emitted("markRendered")).toHaveLength(1);
  });

  it("disables the header action when the page holds no unread rows", () => {
    const wrapper = render({
      entries: [makeRow(2, { is_read: true }), makeRow(1, { is_read: true })],
    });
    expect(wrapper.find(".mark-rendered").attributes("disabled")).toBeDefined();
  });

  it("offers neither action when the list is not a triage list", () => {
    const wrapper = render({
      entries: mixedRows(),
      view: "all",
      batchActionsAvailable: false,
    });
    expect(wrapper.find(".mark-rendered").exists()).toBe(false);
    expect(markAboveButtons(wrapper)).toHaveLength(0);
  });

  it("labels each row action with the unread entries above it", () => {
    const wrapper = render({ entries: mixedRows() });
    const labels = markAboveButtons(wrapper).map((b) => b.text());
    // Row 0 has nothing above it; row 1 has one unread above; rows 2 and 3
    // both see the two unread entries at the top of the list.
    expect(labels).toEqual(["↑1", "↑2", "↑2"]);
  });

  it("emits the row index so the app can slice the loaded rows", async () => {
    const wrapper = render({ entries: mixedRows() });
    await markAboveButtons(wrapper)[1].trigger("click");
    expect(wrapper.emitted("markAbove")).toEqual([[2]]);
  });

  it("does not open the entry whose row action was used", async () => {
    const wrapper = render({ entries: mixedRows() });
    await markAboveButtons(wrapper)[0].trigger("click");
    expect(wrapper.emitted("select")).toBeUndefined();
  });
});
