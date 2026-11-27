import { describe, expect, it, vi } from "vitest";
import { defineComponent, h, nextTick } from "vue";
import { mount } from "@vue/test-utils";
import {
  NARROW_MAX_WIDTH,
  NARROW_MEDIA_QUERY,
  layoutOf,
  useLayout,
  watchLayout,
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
