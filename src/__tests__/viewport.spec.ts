import { describe, expect, it, vi } from "vitest";
import { defineComponent, h, nextTick } from "vue";
import { mount } from "@vue/test-utils";
import {
  COARSE_MEDIA_QUERY,
  COARSE_NARROW_MAX_WIDTH,
  COARSE_NARROW_MEDIA_QUERY,
  NARROW_MAX_WIDTH,
  NARROW_MEDIA_QUERY,
  inputOf,
  layoutOf,
  useLayout,
  useViewport,
  watchLayout,
  watchViewport,
  type MediaQueryLike,
} from "../lib/viewport";

/** A MediaQueryList stand-in whose `matches` the test can flip. */
class FakeMediaQuery implements MediaQueryLike {
  matches: boolean;
  readonly listeners = new Set<() => void>();
  readonly added: string[] = [];
  readonly removed: string[] = [];

  constructor(matches: boolean) {
    this.matches = matches;
  }

  addEventListener(type: "change", listener: () => void) {
    this.added.push(type);
    this.listeners.add(listener);
  }

  removeEventListener(type: "change", listener: () => void) {
    this.removed.push(type);
    this.listeners.delete(listener);
  }

  /** Simulate the viewport crossing the breakpoint. */
  resize(matches: boolean) {
    this.matches = matches;
    for (const listener of this.listeners) listener();
  }
}

function root() {
  return document.documentElement;
}

describe("viewport", () => {
  it("breaks at a width below the desktop minimum window size", () => {
    expect(NARROW_MAX_WIDTH).toBe(800);
    expect(NARROW_MAX_WIDTH).toBeLessThan(900);
    expect(NARROW_MEDIA_QUERY).toBe("(max-width: 800px)");
  });

  it("maps a matching query to the narrow layout", () => {
    expect(layoutOf(true)).toBe("narrow");
    expect(layoutOf(false)).toBe("wide");
  });

  it("marks the document on subscribe", () => {
    const media = new FakeMediaQuery(true);
    const unwatch = watchLayout({ matchMedia: () => media });
    expect(root().dataset.layout).toBe("narrow");
    unwatch();
  });

  it("toggles the document when the width crosses the breakpoint", () => {
    const media = new FakeMediaQuery(false);
    const seen: string[] = [];
    const unwatch = watchLayout({
      matchMedia: () => media,
      onChange: (layout) => seen.push(layout),
    });
    expect(root().dataset.layout).toBe("wide");

    media.resize(true);
    expect(root().dataset.layout).toBe("narrow");

    media.resize(false);
    expect(root().dataset.layout).toBe("wide");

    expect(seen).toEqual(["wide", "narrow", "wide"]);
    unwatch();
  });

  it("removes its listener when unsubscribed", () => {
    const media = new FakeMediaQuery(false);
    const unwatch = watchLayout({ matchMedia: () => media });
    expect(media.listeners.size).toBe(1);

    unwatch();
    expect(media.listeners.size).toBe(0);
    expect(media.removed).toEqual(["change"]);

    // A late event after unmounting must not touch the document.
    root().dataset.layout = "wide";
    media.resize(true);
    expect(root().dataset.layout).toBe("wide");
  });

  it("falls back to the wide layout without matchMedia support", () => {
    root().dataset.layout = "narrow";
    const unwatch = watchLayout({ matchMedia: () => null });
    expect(root().dataset.layout).toBe("wide");
    expect(() => unwatch()).not.toThrow();
  });

  it("exposes the layout reactively and cleans up on unmount", async () => {
    const media = new FakeMediaQuery(true);
    const Probe = defineComponent({
      setup() {
        const layout = useLayout(() => media);
        return () => h("span", { "data-layout": layout.value });
      },
    });

    const wrapper = mount(Probe);
    await nextTick();
    expect(wrapper.get("span").attributes("data-layout")).toBe("narrow");

    media.resize(false);
    await nextTick();
    expect(wrapper.get("span").attributes("data-layout")).toBe("wide");

    wrapper.unmount();
    expect(media.listeners.size).toBe(0);
  });

  it("does not react to a fake query builder that ignores the argument", () => {
    const spy = vi.fn(() => new FakeMediaQuery(false));
    const unwatch = watchLayout({ matchMedia: spy });
    expect(spy).toHaveBeenCalledWith(NARROW_MEDIA_QUERY);
    unwatch();
  });
});

/** One fake list per query, so the three questions can answer differently —
 *  which is the whole point of having three of them. */
function queries(answers: {
  narrow?: boolean;
  coarseNarrow?: boolean;
  coarse?: boolean;
}): {
  matchMedia: (query: string) => MediaQueryLike | null;
  lists: Map<string, FakeMediaQuery>;
} {
  const lists = new Map<string, FakeMediaQuery>();
  lists.set(NARROW_MEDIA_QUERY, new FakeMediaQuery(answers.narrow ?? false));
  lists.set(
    COARSE_NARROW_MEDIA_QUERY,
    new FakeMediaQuery(answers.coarseNarrow ?? false),
  );
  lists.set(COARSE_MEDIA_QUERY, new FakeMediaQuery(answers.coarse ?? false));
  return { matchMedia: (query: string) => lists.get(query) ?? null, lists };
}

describe("viewport decisions", () => {
  it("keeps the coarse threshold above a landscape phone and below a tablet", () => {
    // The desktop window's minimum is 900, so it must not narrow by pointer.
    expect(COARSE_NARROW_MAX_WIDTH).toBeGreaterThan(900);
    expect(COARSE_NARROW_MEDIA_QUERY).toContain("(pointer: coarse)");
    expect(COARSE_NARROW_MEDIA_QUERY).toContain("max-width: 1023px");
    expect(COARSE_MEDIA_QUERY).toBe("(pointer: coarse)");
  });

  it("answers the four viewport rows of the design", () => {
    // width < 800, any pointer -> one pane, touch
    expect(inputOf({ narrow: true, coarse: false })).toBe("touch");
    // 800..1023 with a coarse pointer -> one pane, touch
    expect(inputOf({ narrow: true, coarse: true })).toBe("touch");
    // >= 1024 with a coarse pointer -> three columns, still touch
    expect(inputOf({ narrow: false, coarse: true })).toBe("touch");
    // >= 800 with a fine pointer -> three columns, pointer
    expect(inputOf({ narrow: false, coarse: false })).toBe("pointer");
  });

  it("keeps three columns on a landscape tablet and gives them touch affordances", () => {
    const { matchMedia } = queries({ narrow: false, coarseNarrow: false, coarse: true });
    const seen: { layout: string; input: string }[] = [];
    const unwatch = watchViewport({
      matchMedia,
      onChange: (state) => seen.push({ ...state }),
    });

    expect(root().dataset.layout).toBe("wide");
    expect(root().dataset.input).toBe("touch");
    unwatch();
  });

  it("shows one pane on a landscape phone", () => {
    // A coarse pointer inside the coarse threshold: the width holds three
    // columns, the pointer type says there is no room to aim at them.
    const { matchMedia } = queries({ narrow: false, coarseNarrow: true, coarse: true });
    const unwatch = watchViewport({ matchMedia });

    expect(root().dataset.layout).toBe("narrow");
    expect(root().dataset.input).toBe("touch");
    unwatch();
  });

  it("shows one pane and a pointer on a narrow desktop window", () => {
    const { matchMedia } = queries({ narrow: true, coarseNarrow: false, coarse: false });
    const unwatch = watchViewport({ matchMedia });

    expect(root().dataset.layout).toBe("narrow");
    expect(root().dataset.input).toBe("touch");
    unwatch();
  });

  it("keeps a desktop window on three columns and a pointer", () => {
    const { matchMedia } = queries({});
    const unwatch = watchViewport({ matchMedia });

    expect(root().dataset.layout).toBe("wide");
    expect(root().dataset.input).toBe("pointer");
    unwatch();
  });

  it("follows the pointer type changing while running", () => {
    const { matchMedia, lists } = queries({});
    const unwatch = watchViewport({ matchMedia });
    expect(root().dataset.input).toBe("pointer");

    lists.get(COARSE_MEDIA_QUERY)?.resize(true);
    expect(root().dataset.input).toBe("touch");
    // The columns did not move: the pointer type does not decide them.
    expect(root().dataset.layout).toBe("wide");

    unwatch();
  });

  it("stops listening on every query when unsubscribed", () => {
    const { matchMedia, lists } = queries({});
    const unwatch = watchViewport({ matchMedia });
    for (const list of lists.values()) expect(list.listeners.size).toBe(1);

    unwatch();
    for (const list of lists.values()) expect(list.listeners.size).toBe(0);
  });

  it("falls back to a wide pointer layout without matchMedia support", () => {
    root().dataset.layout = "narrow";
    root().dataset.input = "touch";
    const unwatch = watchViewport({ matchMedia: () => null });
    expect(root().dataset.layout).toBe("wide");
    expect(root().dataset.input).toBe("pointer");
    expect(() => unwatch()).not.toThrow();
  });

  it("exposes both decisions reactively", async () => {
    const { matchMedia, lists } = queries({});
    const Probe = defineComponent({
      setup() {
        const { layout, input } = useViewport(matchMedia);
        return () =>
          h("span", {
            "data-layout": layout.value,
            "data-input": input.value,
          });
      },
    });

    const wrapper = mount(Probe);
    await nextTick();
    expect(wrapper.get("span").attributes("data-layout")).toBe("wide");
    expect(wrapper.get("span").attributes("data-input")).toBe("pointer");

    lists.get(COARSE_MEDIA_QUERY)?.resize(true);
    await nextTick();
    expect(wrapper.get("span").attributes("data-input")).toBe("touch");
    expect(wrapper.get("span").attributes("data-layout")).toBe("wide");

    wrapper.unmount();
    for (const list of lists.values()) expect(list.listeners.size).toBe(0);
  });
});
