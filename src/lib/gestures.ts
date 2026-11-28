/**
 * The horizontal drags of the touch layout.
 *
 * Two halves, deliberately separate, mirroring how `keymap.ts` splits the
 * grammar from the dispatcher: `beginDrag`/`resolveDrag`/`releaseDrag` decide
 * what a sequence of finger samples means and touch no DOM, while
 * `useHorizontalDrag` feeds them Pointer Events and reports the outcome.
 *
 * The rules the machine encodes, and why each one exists:
 *
 * - A drag only becomes ours once it is clearly horizontal. A finger scrolling
 *   a list wanders sideways a little; claiming that would turn every scroll
 *   into row surgery.
 * - A press that never moves is a tap, so the row it lands on still opens.
 * - Nothing is claimed within the gutter the operating system reserves for its
 *   own edge gestures, or the system back action would stop working on a list
 *   full of rows.
 * - The commit distance scales with the surface, with a floor so a short row
 *   is not too easy to trigger and a ceiling so a wide one is not too far.
 */

import { onBeforeUnmount, ref, type Ref } from "vue";

/** Movement below this is a tap, not a drag. */
export const DRAG_SLOP = 8;

/** Horizontal travel that must be reached before the drag is certainly horizontal. */
export const DIRECTION_LOCK = 10;

/** How much more horizontal than vertical a drag must be to be claimed. */
export const DIRECTION_RATIO = 1.5;

/** Share of the surface's width that commits a drag. */
export const COMMIT_RATIO = 0.4;

/** Floor and ceiling for that commit distance, in CSS pixels. */
export const COMMIT_MIN = 72;
export const COMMIT_MAX = 160;

/** Screen gutter left to the operating system's own edge gestures. */
export const EDGE_GUTTER = 32;

/** How long a motionless press must last to count as a long press. */
export const LONG_PRESS_MS = 500;

/** The pull's own numbers. The commit distance is measured on the resisted
 *  offset, so the list has to travel twice as far under the finger. */
export const PULL_SLOP = 8;
export const PULL_COMMIT = 64;
export const PULL_MAX_OFFSET = 96;
export const PULL_RESISTANCE = 0.5;

export type DragPhase = "pending" | "ignored" | "dragging";

export type DragDirection = "left" | "right";

export interface DragState {
  readonly phase: DragPhase;
  /** Horizontal travel since the press began, signed: negative is leftward. */
  readonly dx: number;
  readonly dy: number;
  /** Travel that commits, fixed when the press began. */
  readonly commitAt: number;
}

/** The travel that commits a drag on a surface `width` pixels wide. */
export function commitDistance(width: number): number {
  return Math.max(
    COMMIT_MIN,
    Math.min(Math.round(width * COMMIT_RATIO), COMMIT_MAX),
  );
}

export function beginDrag(start: {
  /** Where the finger landed, in client coordinates. */
  x: number;
  width: number;
  /** The full width of the screen, for the edge gutter. */
  viewportWidth: number;
  /** Gutter to leave the system; the default unless a surface needs more. */
  gutter?: number;
}): DragState {
  const gutter = start.gutter ?? EDGE_GUTTER;
  const inGutter = start.x < gutter || start.x > start.viewportWidth - gutter;
  return {
    phase: inGutter ? "ignored" : "pending",
    dx: 0,
    dy: 0,
    commitAt: commitDistance(start.width),
  };
}

/**
 * The next state after the finger has travelled `dx`/`dy` from where it
 * landed. Once a drag is claimed it stays claimed, because by then the finger
 * has chosen an axis and the browser is no longer scrolling.
 */
export function resolveDrag(
  state: DragState,
  move: { dx: number; dy: number },
): DragState {
  const { dx, dy } = move;
  if (state.phase === "ignored") return state;

  if (state.phase === "dragging") {
    return { ...state, dx, dy };
  }

  const ax = Math.abs(dx);
  const ay = Math.abs(dy);

  // Still inside the slop: could be a tap.
  if (ax < DRAG_SLOP && ay < DRAG_SLOP) return { ...state, dx, dy };

  // Clearly vertical: a scroll, and never ours again.
  if (ay >= ax * DIRECTION_RATIO) {
    return { ...state, phase: "ignored", dx, dy };
  }

  // Clearly horizontal and past the lock: ours.
  if (ax >= DIRECTION_LOCK && ax > ay * DIRECTION_RATIO) {
    return { ...state, phase: "dragging", dx, dy };
  }

  // Neither yet: wait for the finger to commit to an axis.
  return { ...state, dx, dy };
}

/** The action the drag would commit if released now, or null while undecided. */
export function dragIntent(state: DragState): DragDirection | null {
  if (state.phase !== "dragging") return null;
  return state.dx < 0 ? "left" : "right";
}

/** What a release commits: a direction, or nothing for a short drag. */
export function releaseDrag(state: DragState): DragDirection | null {
  const intent = dragIntent(state);
  if (intent === null) return null;
  return Math.abs(state.dx) >= state.commitAt ? intent : null;
}

/* --- The pull at the top of a list --------------------------------------- */

export type PullPhase = "pending" | "ignored" | "pulling";

export interface PullState {
  readonly phase: PullPhase;
  /** Downward travel since the press began, unsigned. */
  readonly dy: number;
}

/**
 * A press on a list that is already at its top may become a pull. Anywhere else
 * the press belongs to scrolling, which is why this is decided at touch-down
 * rather than later: once the browser has started scrolling, no amount of
 * pointer samples can take the gesture back.
 */
export function beginPull(start: { atTop: boolean }): PullState {
  return { phase: start.atTop ? "pending" : "ignored", dy: 0 };
}

/**
 * Decide what a movement means. Downward travel is the pull; sideways travel is
 * the row's drag; upward travel is scrolling, which the list keeps.
 */
export function resolvePull(
  state: PullState,
  sample: { dx: number; dy: number },
): PullState {
  if (state.phase === "ignored") return state;
  const { dx, dy } = sample;
  if (dy < 0) return { phase: "ignored", dy: 0 };
  if (Math.abs(dy) < PULL_SLOP) return { phase: state.phase, dy: Math.max(dy, 0) };
  if (Math.abs(dx) >= Math.abs(dy) * DIRECTION_RATIO && Math.abs(dx) > DIRECTION_LOCK) {
    // Clearly sideways: this is a row action, not a pull.
    return { phase: "ignored", dy: 0 };
  }
  return { phase: "pulling", dy };
}

/** How far the list follows the finger: resisted, and never past the cap. */
export function pullOffset(state: PullState): number {
  if (state.phase !== "pulling") return 0;
  return Math.min(state.dy * PULL_RESISTANCE, PULL_MAX_OFFSET);
}

/** Whether releasing now refreshes. */
export function releasePull(state: PullState): boolean {
  return pullOffset(state) >= PULL_COMMIT;
}

export interface PullOptions {
  /** False where no pull should exist: a pointer layout, or the reading pane. */
  enabled: () => boolean;
  /** The scroll port whose top is the only place a pull can begin. */
  port: () => HTMLElement | null;
  /** Released past the threshold. */
  onRefresh: () => void;
}

export interface PullToRefresh {
  /** Travel of the list while the finger is pulling, in pixels. */
  offset: Ref<number>;
  /** True once the movement has been claimed. */
  pulling: Ref<boolean>;
  /** Bind to the scroll port. All four, because the pull owns the whole move. */
  onTouchStart: (event: TouchEvent) => void;
  onTouchMove: (event: TouchEvent) => void;
  onTouchEnd: (event: TouchEvent) => void;
  onTouchCancel: (event: TouchEvent) => void;
}

/** The first touch of an event, whichever list the browser put it in. */
function firstTouch(event: TouchEvent): Touch | null {
  return event.touches[0] ?? event.changedTouches[0] ?? null;
}

/**
 * The pull, driven by touch events rather than pointer events.
 *
 * Deliberately: inside a scroll port the browser claims a vertical pan and
 * sends `pointercancel` to whatever was following it, so a pointer-based pull
 * never sees enough travel to commit — the list rubber-bands instead. Claiming
 * the gesture back takes a non-passive `touchmove` that calls
 * `preventDefault`, which is also why this machine is separate from the row
 * drags (they only ever want the horizontal axis, which `touch-action: pan-y`
 * already leaves to them).
 *
 * Nothing is prevented until the pull is actually claimed: a sideways drag, an
 * upward drag, or a press on a scrolled list leaves the browser's own handling
 * completely alone.
 */
export function usePullToRefresh(options: PullOptions): PullToRefresh {
  const offset = ref(0);
  const pulling = ref(false);

  let state: PullState | null = null;
  let origin = { x: 0, y: 0 };

  function reset() {
    state = null;
    offset.value = 0;
    pulling.value = false;
  }

  function onTouchStart(event: TouchEvent) {
    if (!options.enabled()) return;
    // A second finger is a pinch or a two-finger scroll, not a pull.
    if (event.touches.length !== 1) {
      reset();
      return;
    }
    const port = options.port();
    const touch = firstTouch(event);
    if (!port || !touch) return;
    origin = { x: touch.clientX, y: touch.clientY };
    state = beginPull({ atTop: port.scrollTop <= 0 });
  }

  function onTouchMove(event: TouchEvent) {
    if (state === null) return;
    const touch = firstTouch(event);
    if (!touch) return;
    const resolved = resolvePull(state, {
      dx: touch.clientX - origin.x,
      dy: touch.clientY - origin.y,
    });
    if (resolved.phase === "ignored") {
      if (resolved !== state) reset();
      state = null;
      return;
    }
    state = resolved;
    if (resolved.phase !== "pulling") return;
    // Claim the gesture from the browser's scrolling, and only now.
    if (event.cancelable) event.preventDefault();
    offset.value = pullOffset(resolved);
    pulling.value = true;
  }

  function finish(commit: boolean) {
    if (state === null) return;
    const released = commit && releasePull(state);
    reset();
    if (released) options.onRefresh();
  }

  function onTouchEnd() {
    finish(true);
  }

  function onTouchCancel() {
    finish(false);
  }

  onBeforeUnmount(reset);

  return { offset, pulling, onTouchStart, onTouchMove, onTouchEnd, onTouchCancel };
}

/** What the finger is dragging. The composable never interprets it. */
export interface DragTarget {
  key: string | number;
}

export interface HorizontalDragOptions {
  /** Commit of a drag that passed the threshold. */
  onCommit: (target: DragTarget, direction: DragDirection) => void;
  /** A motionless press, for the surface's own menu. */
  onLongPress?: (target: DragTarget) => void;
  /** Gutter to leave the system; the reading pane doubles it. */
  gutter?: number;
  /** False on a pointer layout, where no drag is ever claimed. */
  enabled: () => boolean;
  /** Called whenever the drag itself starts or ends, for styling. */
  onStart?: (target: DragTarget) => void;
  onEnd?: (target: DragTarget) => void;
}

export interface HorizontalDrag {
  /** The target under the finger right now, if any. */
  activeKey: Ref<string | number | null>;
  /** Its travel, in pixels, signed. */
  offset: Ref<number>;
  /** True while a claimed drag is in progress. */
  dragging: Ref<boolean>;
  /** Bind to the surface that should be draggable. */
  onPointerDown: (event: PointerEvent, target: DragTarget) => void;
}

function screenWidth(): number {
  if (typeof window === "undefined") return 0;
  return window.innerWidth || 0;
}

/**
 * Feed Pointer Events into the state machine above.
 *
 * Passive listeners throughout, and `touch-action: pan-y` in CSS: the browser
 * keeps vertical scrolling native, so no `preventDefault` is needed and no
 * drag can make the list stutter. A commit is reported once, on release.
 */
export function useHorizontalDrag(
  options: HorizontalDragOptions,
): HorizontalDrag {
  const activeKey = ref<string | number | null>(null);
  const offset = ref(0);
  const dragging = ref(false);

  let state: DragState | null = null;
  let target: DragTarget | null = null;
  let origin = { x: 0, y: 0 };
  let longPress: ReturnType<typeof setTimeout> | null = null;
  /**
   * True between a drag and the click the browser synthesises after it. The
   * click is swallowed once, so a drag never also opens the entry — and only
   * until the end of the current task, which is all the lifetime it needs.
   */
  let suppressClick = false;

  function clearLongPress() {
    if (longPress !== null) {
      clearTimeout(longPress);
      longPress = null;
    }
  }

  function reset() {
    clearLongPress();
    state = null;
    target = null;
    dragging.value = false;
    offset.value = 0;
    activeKey.value = null;
  }

  function onPointerDown(event: PointerEvent, next: DragTarget) {
    if (!options.enabled()) return;
    // Primary button only: a right-click is not a drag.
    if (event.button > 0) return;
    // A fresh touchdown means whatever was in flight is over. Ignoring it
    // instead would leave a surface stuck wherever a lost pointerup dropped it
    // — and, because the next drag would be ignored too, every later attempt
    // would do nothing until the list was rebuilt.
    if (state !== null) reset();

    const surface = event.currentTarget as HTMLElement | null;
    const width = surface?.getBoundingClientRect?.().width ?? 0;
    origin = { x: event.clientX, y: event.clientY };
    target = next;
    activeKey.value = next.key;
    state = beginDrag({
      x: event.clientX,
      width,
      viewportWidth: screenWidth(),
      gutter: options.gutter,
    });

    if (options.onLongPress) {
      clearLongPress();
      longPress = setTimeout(() => {
        longPress = null;
        const held = target;
        // A held press never becomes a drag, even if the finger wanders: the
        // menu it opens owns the surface until it closes.
        reset();
        if (held) options.onLongPress?.(held);
      }, LONG_PRESS_MS);
    }
  }

  function onPointerMove(event: PointerEvent) {
    if (state === null) return;
    const dx = event.clientX - origin.x;
    const dy = event.clientY - origin.y;
    if (longPress !== null && (Math.abs(dx) > DRAG_SLOP || Math.abs(dy) > DRAG_SLOP)) {
      clearLongPress();
    }
    state = resolveDrag(state, { dx, dy });
    if (state.phase === "dragging") {
      offset.value = state.dx;
      if (!dragging.value) {
        dragging.value = true;
        if (target) options.onStart?.(target);
      }
    }
  }

  function finish(commit: boolean) {
    if (state === null) return;
    const direction = commit ? releaseDrag(state) : null;
    const ended = target;
    const wasDragging = dragging.value;
    reset();
    if (direction !== null && ended) options.onCommit(ended, direction);
    if (wasDragging && ended) options.onEnd?.(ended);
    if (wasDragging) {
      suppressClick = true;
      // Backstop: the synthesised click arrives before this runs, but a
      // surface without one must not keep swallowing the next tap.
      setTimeout(() => {
        suppressClick = false;
      }, 0);
    }
  }

  function onPointerUp() {
    finish(true);
  }

  function onPointerCancel() {
    finish(false);
  }

  function onClick(event: MouseEvent) {
    if (!suppressClick) return;
    suppressClick = false;
    event.stopPropagation();
    event.preventDefault();
  }

  if (typeof window !== "undefined") {
    window.addEventListener("pointermove", onPointerMove, { passive: true });
    window.addEventListener("pointerup", onPointerUp, { passive: true });
    window.addEventListener("pointercancel", onPointerCancel, { passive: true });
    // Capture, so the surface's own click handler never sees a suppressed tap.
    window.addEventListener("click", onClick, true);
  }

  onBeforeUnmount(() => {
    reset();
    if (typeof window === "undefined") return;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", onPointerCancel);
    window.removeEventListener("click", onClick, true);
  });

  return { activeKey, offset, dragging, onPointerDown };
}
