import { describe, expect, it, vi } from "vitest";
import { createBackStack, type HistoryLike, type PopTargetLike } from "../lib/back-stack";

/** A history the test can inspect, and whose pops it can deliver by hand. */
class FakeHistory implements HistoryLike {
  readonly pushed: unknown[] = [];
  readonly gone: number[] = [];

  pushState(state: unknown) {
    this.pushed.push(state);
  }

  go(delta: number) {
    this.gone.push(delta);
  }
}

class FakeWindow implements PopTargetLike {
  private listeners = new Set<() => void>();

  addEventListener(_type: "popstate", listener: () => void) {
    this.listeners.add(listener);
  }

  removeEventListener(_type: "popstate", listener: () => void) {
    this.listeners.delete(listener);
  }

  /** Deliver the system back action. */
  back() {
    for (const listener of this.listeners) listener();
  }

  get attached() {
    return this.listeners.size;
  }
}

function build() {
  const history = new FakeHistory();
  const target = new FakeWindow();
  const onBack = vi.fn();
  const stack = createBackStack({ onBack, history, target });
  return { history, target, onBack, stack };
}

describe("back stack", () => {
  it("mirrors one history entry per open layer", () => {
    const { history, stack } = build();

    stack.sync(1);
    stack.sync(2);

    expect(history.pushed).toEqual([{ backLayer: 1 }, { backLayer: 2 }]);
    expect(history.gone).toEqual([]);
    expect(stack.depth()).toBe(2);
  });

  it("ignores a sync that does not change the depth", () => {
    const { history, stack } = build();
    stack.sync(1);
    stack.sync(1);
    expect(history.pushed).toHaveLength(1);
    expect(history.gone).toEqual([]);
  });

  it("asks the app to close a layer when the system back action arrives", () => {
    const { stack, target, onBack, history } = build();
    stack.sync(2);

    target.back();
    expect(onBack).toHaveBeenCalledTimes(1);
    // The history already moved; the stack must not move it again.
    expect(history.gone).toEqual([]);

    // The app then closes the layer it reported.
    stack.sync(1);
    expect(stack.depth()).toBe(1);
    expect(history.gone).toEqual([]);
  });

  it("unwinds one layer per back action, in order", () => {
    const { stack, target, onBack } = build();
    stack.sync(3);

    target.back();
    stack.sync(2);
    target.back();
    stack.sync(1);
    target.back();
    stack.sync(0);

    expect(onBack).toHaveBeenCalledTimes(3);
    expect(stack.depth()).toBe(0);
  });

  it("drops its own entries when a layer is closed by a control", () => {
    const { stack, history } = build();
    stack.sync(2);

    stack.sync(0);

    expect(history.gone).toEqual([-2]);
    expect(stack.depth()).toBe(0);
  });

  it("does not close a layer for the pops it caused itself", () => {
    const { stack, history, target, onBack } = build();
    stack.sync(1);
    stack.sync(0); // a close button: one pop of ours is on its way
    expect(history.gone).toEqual([-1]);

    target.back();
    expect(onBack).not.toHaveBeenCalled();
  });

  it("does nothing at the top level, leaving the system to leave the app", () => {
    const { stack, target, onBack, history } = build();
    target.back();
    expect(onBack).not.toHaveBeenCalled();

    stack.sync(0);
    expect(history.pushed).toEqual([]);
    expect(history.gone).toEqual([]);
  });

  it("stops listening once disposed", () => {
    const { stack, target, onBack } = build();
    stack.sync(1);
    expect(target.attached).toBe(1);

    stack.dispose();
    expect(target.attached).toBe(0);

    target.back();
    expect(onBack).not.toHaveBeenCalled();
  });

  it("treats a negative depth as none", () => {
    const { history, stack } = build();
    stack.sync(-3);
    expect(stack.depth()).toBe(0);
    expect(history.pushed).toEqual([]);
  });
});
