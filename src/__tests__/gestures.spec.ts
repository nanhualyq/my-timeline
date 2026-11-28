import { describe, expect, it, vi } from "vitest";
import { defineComponent, h, nextTick } from "vue";
import { mount } from "@vue/test-utils";
import {
  COMMIT_MAX,
  COMMIT_MIN,
  COMMIT_RATIO,
  DIRECTION_LOCK,
  DRAG_SLOP,
  EDGE_GUTTER,
  PULL_MAX_OFFSET,
  beginDrag,
  beginPull,
  commitDistance,
  dragIntent,
  pullOffset,
  releaseDrag,
  releasePull,
  resolveDrag,
  resolvePull,
  useHorizontalDrag,
  type DragState,
} from "../lib/gestures";

/** A surface 360 px wide: a phone row. */
const ROW_WIDTH = 360;
const VIEWPORT = 412;

function start(x = 200, width = ROW_WIDTH): DragState {
  return beginDrag({ x, width, viewportWidth: VIEWPORT });
}

/** Feed a run of samples through the machine and return the final state. */
function drag(state: DragState, samples: [number, number][]): DragState {
  return samples.reduce(
    (acc, [dx, dy]) => resolveDrag(acc, { dx, dy }),
    state,
  );
}

describe("gesture thresholds", () => {
  it("scales the commit distance with the surface", () => {
    // 40% of a phone row, well inside the floor and the ceiling.
    expect(COMMIT_RATIO).toBe(0.4);
    expect(commitDistance(ROW_WIDTH)).toBe(Math.round(ROW_WIDTH * 0.4));
    // A surface too narrow to have a sensible 40% is not made easier to trigger.
    expect(commitDistance(100)).toBe(COMMIT_MIN);
    // A wide surface does not demand a longer drag than a finger can reach.
    expect(commitDistance(1200)).toBe(COMMIT_MAX);
  });

  it("starts pending, with the commit distance already fixed", () => {
    const state = start();
    expect(state.phase).toBe("pending");
    expect(state.commitAt).toBe(commitDistance(ROW_WIDTH));
    expect(dragIntent(state)).toBeNull();
    expect(releaseDrag(state)).toBeNull();
  });
});

describe("resolving a drag", () => {
  it("treats a press that does not move as a tap", () => {
    const state = drag(start(), [
      [0, 0],
      [3, 2],
    ]);
    expect(state.phase).toBe("pending");
    expect(releaseDrag(state)).toBeNull();
  });

  it("leaves a vertical drag to the list", () => {
    const state = drag(start(), [
      [1, 20],
      [4, 60],
    ]);
    expect(state.phase).toBe("ignored");
    // Once the axis is chosen, wandering sideways cannot reclaim it.
    const later = drag(state, [[-120, 4]]);
    expect(later.phase).toBe("ignored");
    expect(releaseDrag(later)).toBeNull();
  });

  it("leaves a mostly-vertical drag to the list", () => {
    const state = drag(start(), [
      [10, 30],
      [18, 45],
    ]);
    expect(state.phase).toBe("ignored");
  });

  it("claims a drag once it is clearly horizontal", () => {
    const state = drag(start(), [[DIRECTION_LOCK, 2]]);
    expect(state.phase).toBe("dragging");
    expect(dragIntent(state)).toBe("right");
  });

  it("waits while the finger has not chosen an axis", () => {
    // Past the slop, but no axis is dominant yet.
    const state = drag(start(), [[DRAG_SLOP, DRAG_SLOP]]);
    expect(state.phase).toBe("pending");
  });

  it("reads leftward travel as the leftward action", () => {
    const state = drag(start(), [
      [-20, 1],
      [-150, 2],
    ]);
    expect(dragIntent(state)).toBe("left");
    expect(releaseDrag(state)).toBe("left");
  });

  it("reads rightward travel as the rightward action", () => {
    const state = drag(start(), [[150, 0]]);
    expect(dragIntent(state)).toBe("right");
    expect(releaseDrag(state)).toBe("right");
  });

  it("commits nothing below the threshold", () => {
    const state = drag(start(), [[-(COMMIT_MIN + 1), 0]]);
    expect(state.phase).toBe("dragging");
    // Past the floor, but short of this surface's own commit distance.
    expect(state.commitAt).toBeGreaterThan(COMMIT_MIN + 1);
    expect(releaseDrag(state)).toBeNull();
  });

  it("commits the direction the finger ends in, not the one it passed through", () => {
    const state = drag(start(), [
      [-150, 2],
      [40, 2],
      [150, 2],
    ]);
    expect(dragIntent(state)).toBe("right");
    // The abandoned leftward action is not what a release commits.
    expect(releaseDrag(state)).toBe("right");
  });

  it("cancels a drag released back across its own threshold", () => {
    const state = drag(start(), [
      [-150, 2],
      [10, 2],
    ]);
    expect(releaseDrag(state)).toBeNull();
  });
});

describe("edge gutter", () => {
  it("refuses a drag that begins in the system gutter", () => {
    const state = beginDrag({ x: 4, width: ROW_WIDTH, viewportWidth: VIEWPORT });
    expect(state.phase).toBe("ignored");
    expect(drag(state, [[-200, 0]]).phase).toBe("ignored");
    expect(releaseDrag(drag(state, [[-200, 0]]))).toBeNull();
  });

  it("refuses one that begins in the far gutter too", () => {
    const state = beginDrag({
      x: VIEWPORT - 2,
      width: ROW_WIDTH,
      viewportWidth: VIEWPORT,
    });
    expect(state.phase).toBe("ignored");
  });

  it("claims one that begins just inside the gutter's edge", () => {
    const state = beginDrag({
      x: EDGE_GUTTER,
      width: ROW_WIDTH,
      viewportWidth: VIEWPORT,
    });
    expect(state.phase).toBe("pending");
  });
});

/** The smallest component that can host the composable. */
function probe(options: {
  enabled?: () => boolean;
  onCommit?: (target: { key: string | number }, direction: string) => void;
  onLongPress?: (target: { key: string | number }) => void;
  onStart?: (target: { key: string | number }) => void;
  onEnd?: (target: { key: string | number }) => void;
}) {
  return defineComponent({
    setup() {
      const drag = useHorizontalDrag({
        enabled: options.enabled ?? (() => true),
        onCommit: options.onCommit ?? (() => {}),
        onLongPress: options.onLongPress,
        onStart: options.onStart,
        onEnd: options.onEnd,
      });
      return () =>
        h(
          "div",
          {
            class: "surface",
            "data-offset": drag.offset.value,
            "data-dragging": drag.dragging.value ? "yes" : "no",
            onPointerdown: (event: PointerEvent) =>
              drag.onPointerDown(event, { key: 7 }),
          },
          "row",
        );
    },
  });
}

/** jsdom has no PointerEvent, and the composable only reads fields MouseEvent
 *  also carries, so a MouseEvent named `pointerdown` drives the same path. */
function press(target: EventTarget, type: string, x: number, y: number) {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    clientX: x,
    clientY: y,
  });
  target.dispatchEvent(event);
  return event;
}

describe("useHorizontalDrag", () => {
  it("commits once and reports the travel while it lasts", async () => {
    const commits: string[] = [];
    const wrapper = mount(probe({ onCommit: (_t, direction) => commits.push(direction) }));
    const surface = wrapper.get(".surface");

    press(surface.element, "pointerdown", 200, 300);
    press(window, "pointermove", 100, 302);
    await nextTick();
    expect(surface.attributes("data-offset")).toBe("-100");
    expect(surface.attributes("data-dragging")).toBe("yes");

    press(window, "pointermove", 40, 302);
    press(window, "pointerup", 40, 302);
    await nextTick();

    expect(commits).toEqual(["left"]);
    expect(surface.attributes("data-dragging")).toBe("no");
    expect(surface.attributes("data-offset")).toBe("0");
    wrapper.unmount();
  });

  it("commits nothing for a tap", async () => {
    const commits: string[] = [];
    const wrapper = mount(probe({ onCommit: (_t, direction) => commits.push(direction) }));

    press(wrapper.get(".surface").element, "pointerdown", 200, 300);
    press(window, "pointerup", 201, 301);

    expect(commits).toEqual([]);
    wrapper.unmount();
  });

  it("does not claim a drag when it is disabled", async () => {
    const commits: string[] = [];
    const wrapper = mount(
      probe({ enabled: () => false, onCommit: (_t, d) => commits.push(d) }),
    );

    press(wrapper.get(".surface").element, "pointerdown", 200, 300);
    press(window, "pointermove", 40, 300);
    press(window, "pointerup", 40, 300);

    expect(commits).toEqual([]);
    expect(wrapper.get(".surface").attributes("data-dragging")).toBe("no");
    wrapper.unmount();
  });

  it("swallows the click a committed drag would otherwise become", async () => {
    const opened = vi.fn();
    const Host = defineComponent({
      setup() {
        const drag = useHorizontalDrag({ enabled: () => true, onCommit: () => {} });
        return () =>
          h(
            "div",
            {
              class: "surface",
              onClick: opened,
              onPointerdown: (event: PointerEvent) =>
                drag.onPointerDown(event, { key: 1 }),
            },
            "row",
          );
      },
    });
    // Attached: a click only reaches window's capture listener when the
    // surface is in the document, which is what a real row always is.
    const wrapper = mount(Host, { attachTo: document.body });

    press(wrapper.get(".surface").element, "pointerdown", 200, 300);
    press(window, "pointermove", 40, 300);
    press(window, "pointerup", 40, 300);

    // What the browser synthesises right after the drag.
    wrapper.get(".surface").element.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true }),
    );

    expect(opened).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("still lets a plain tap open the surface", async () => {
    const opened = vi.fn();
    const Host = defineComponent({
      setup() {
        const drag = useHorizontalDrag({ enabled: () => true, onCommit: () => {} });
        return () =>
          h(
            "div",
            {
              class: "surface",
              onClick: opened,
              onPointerdown: (event: PointerEvent) =>
                drag.onPointerDown(event, { key: 1 }),
            },
            "row",
          );
      },
    });
    // Attached: a click only reaches window's capture listener when the
    // surface is in the document, which is what a real row always is.
    const wrapper = mount(Host, { attachTo: document.body });

    press(wrapper.get(".surface").element, "pointerdown", 200, 300);
    press(window, "pointerup", 200, 300);
    wrapper.get(".surface").element.dispatchEvent(
      new MouseEvent("click", { bubbles: true, cancelable: true }),
    );

    expect(opened).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });

  it("reports a long press when the finger does not move", async () => {
    vi.useFakeTimers();
    const held: (string | number)[] = [];
    const wrapper = mount(probe({ onLongPress: (t) => held.push(t.key) }));

    press(wrapper.get(".surface").element, "pointerdown", 200, 300);
    vi.advanceTimersByTime(600);
    vi.useRealTimers();

    expect(held).toEqual([7]);
    wrapper.unmount();
  });

  it("does not report a long press once the finger moves", async () => {
    vi.useFakeTimers();
    const held: (string | number)[] = [];
    const wrapper = mount(probe({ onLongPress: (t) => held.push(t.key) }));

    press(wrapper.get(".surface").element, "pointerdown", 200, 300);
    press(window, "pointermove", 150, 300);
    vi.advanceTimersByTime(600);
    vi.useRealTimers();

    expect(held).toEqual([]);
    wrapper.unmount();
  });

  it("mounts and unmounts without Pointer Events support", async () => {
    const original = (window as { PointerEvent?: unknown }).PointerEvent;
    // A webview old enough to have no Pointer Events at all: nothing may throw.
    delete (window as { PointerEvent?: unknown }).PointerEvent;
    try {
      const wrapper = mount(probe({}));
      expect(wrapper.get(".surface").text()).toBe("row");
      wrapper.unmount();
    } finally {
      if (original !== undefined) {
        (window as { PointerEvent?: unknown }).PointerEvent = original;
      }
    }
  });

  it("cancels a drag whose release never arrived", async () => {
    const commits: string[] = [];
    const wrapper = mount(probe({ onCommit: (_t, d) => commits.push(d) }));
    const surface = wrapper.get(".surface");

    // A gesture that moved but never reported an end: the surface is stuck
    // mid-drag and the next press has to recover from it.
    press(surface.element, "pointerdown", 200, 300);
    press(window, "pointermove", 150, 300);
    await nextTick();
    expect(surface.attributes("data-dragging")).toBe("yes");

    press(surface.element, "pointerdown", 200, 300);
    await nextTick();
    expect(surface.attributes("data-dragging")).toBe("no");
    expect(surface.attributes("data-offset")).toBe("0");

    press(window, "pointermove", 40, 300);
    press(window, "pointerup", 40, 300);
    await nextTick();
    expect(commits).toEqual(["left"]);
    wrapper.unmount();
  });

  it("drops its window listeners when it goes away", async () => {
    const commits: string[] = [];
    const wrapper = mount(probe({ onCommit: (_t, d) => commits.push(d) }));
    wrapper.unmount();

    press(window, "pointermove", 100, 300);
    press(window, "pointerup", 100, 300);

    expect(commits).toEqual([]);
  });
});

describe("the pull machine", () => {
  it("only becomes a pull when the list is already at its top", () => {
    const atTop = resolvePull(beginPull({ atTop: true }), { dx: 0, dy: 40 });
    const scrolled = resolvePull(beginPull({ atTop: false }), { dx: 0, dy: 40 });

    expect(atTop.phase).toBe("pulling");
    expect(scrolled.phase).toBe("ignored");
  });

  it("stays undecided below the movement slop", () => {
    const state = resolvePull(beginPull({ atTop: true }), { dx: 0, dy: 3 });
    expect(state.phase).toBe("pending");
  });

  it("is not a pull when the movement is sideways", () => {
    const state = resolvePull(beginPull({ atTop: true }), { dx: 40, dy: 10 });
    expect(state.phase).toBe("ignored");
  });

  it("is not a pull when the finger goes up", () => {
    const state = resolvePull(beginPull({ atTop: true }), { dx: 0, dy: -40 });
    expect(state.phase).toBe("ignored");
  });

  it("resists the travel and never passes its cap", () => {
    const state = resolvePull(beginPull({ atTop: true }), { dx: 0, dy: 100 });
    expect(pullOffset(state)).toBe(100 * 0.5);
    const far = resolvePull(beginPull({ atTop: true }), { dx: 0, dy: 10_000 });
    expect(pullOffset(far)).toBe(PULL_MAX_OFFSET);
  });

  it("commits only past the threshold", () => {
    const short = resolvePull(beginPull({ atTop: true }), { dx: 0, dy: 80 });
    const long = resolvePull(beginPull({ atTop: true }), { dx: 0, dy: 200 });

    expect(releasePull(short)).toBe(false);
    expect(releasePull(long)).toBe(true);
  });
});
