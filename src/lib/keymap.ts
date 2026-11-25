/**
 * The key map: one table of bindings, and a resolver that turns a keystroke
 * into an action.
 *
 * The table is the single source of truth. Both the dispatcher in `App.vue`
 * and the help overlay read it, so a rebound key cannot leave the overlay
 * describing something the application no longer does.
 *
 * The resolver is deliberately free of timers and DOM access: a prefix that
 * never completes is the container's problem (see `App.vue`), and a keystroke
 * arrives already reduced to a `Keystroke`. That keeps the grammar testable
 * without mounting anything.
 */

/** A keystroke, reduced to the parts the key map cares about. */
export interface Keystroke {
  /** `KeyboardEvent.key`: a printable character, or a named key like "Escape". */
  key: string;
  ctrl?: boolean;
  alt?: boolean;
  meta?: boolean;
  shift?: boolean;
}

export type Action =
  | "cursor-down"
  | "cursor-up"
  | "cursor-top"
  | "cursor-bottom"
  | "half-page-down"
  | "half-page-up"
  | "toggle-read"
  | "toggle-star"
  | "open-link"
  | "toggle-view"
  | "mark-rendered"
  | "refresh-all"
  | "go-all"
  | "go-starred"
  | "pane-scroll-down"
  | "pane-scroll-up"
  | "toggle-help";

/** Overlay grouping, in the order the overlay shows them. */
export const SECTIONS = [
  "Cursor",
  "Entry",
  "List",
  "Go to",
  "Reading pane",
  "Help",
] as const;

export type Section = (typeof SECTIONS)[number];

export interface Binding {
  /** Normalized tokens, e.g. `["g", "g"]` or `["ctrl+d"]`. */
  keys: string[];
  action: Action;
  /** How the key is written where the reader sees it. */
  label: string;
  section: Section;
  description: string;
  /** True when a numeric prefix changes what the action reaches. */
  counts?: boolean;
}

export const BINDINGS: readonly Binding[] = [
  {
    keys: ["j"],
    action: "cursor-down",
    label: "j",
    section: "Cursor",
    description: "Move down: opens the entry and marks it read",
    counts: true,
  },
  {
    keys: ["k"],
    action: "cursor-up",
    label: "k",
    section: "Cursor",
    description: "Move up",
    counts: true,
  },
  {
    keys: ["g", "g"],
    action: "cursor-top",
    label: "gg",
    section: "Cursor",
    description: "First row",
  },
  {
    keys: ["G"],
    action: "cursor-bottom",
    label: "G",
    section: "Cursor",
    description: "Last row, or row N when a count is given",
    counts: true,
  },
  {
    keys: ["ctrl+d"],
    action: "half-page-down",
    label: "Ctrl-d",
    section: "Cursor",
    description: "Half a page down",
    counts: true,
  },
  {
    keys: ["ctrl+u"],
    action: "half-page-up",
    label: "Ctrl-u",
    section: "Cursor",
    description: "Half a page up",
    counts: true,
  },
  {
    keys: ["m"],
    action: "toggle-read",
    label: "m",
    section: "Entry",
    description: "Toggle read / unread on the cursor row",
  },
  {
    keys: ["s"],
    action: "toggle-star",
    label: "s",
    section: "Entry",
    description: "Star or unstar the cursor row",
  },
  {
    keys: ["o"],
    action: "open-link",
    label: "o",
    section: "Entry",
    description: "Open the original article in the browser",
  },
  {
    keys: ["v"],
    action: "toggle-view",
    label: "v",
    section: "List",
    description: "Switch between the All and Unread tabs",
  },
  {
    keys: ["A"],
    action: "mark-rendered",
    label: "A",
    section: "List",
    description: "Mark the unread entries on this page read",
  },
  {
    keys: ["r"],
    action: "refresh-all",
    label: "r",
    section: "List",
    description: "Refresh all feeds",
  },
  {
    keys: ["g", "a"],
    action: "go-all",
    label: "ga",
    section: "Go to",
    description: "All entries",
  },
  {
    keys: ["g", "s"],
    action: "go-starred",
    label: "gs",
    section: "Go to",
    description: "Starred",
  },
  {
    keys: ["J"],
    action: "pane-scroll-down",
    label: "J",
    section: "Reading pane",
    description: "Scroll the article down one screen",
  },
  {
    keys: ["K"],
    action: "pane-scroll-up",
    label: "K",
    section: "Reading pane",
    description: "Scroll the article up one screen",
  },
  {
    keys: ["?"],
    action: "toggle-help",
    label: "?",
    section: "Help",
    description: "Show or hide this key map",
  },
];

/** Tokens that begin a multi-key binding. Derived, so adding `["g", "x"]`
 *  needs no second edit. */
const PREFIXES = new Set(
  BINDINGS.filter((b) => b.keys.length > 1).map((b) => b.keys[0]),
);

/** What the prefix and count state is between two keystrokes. */
export interface KeyState {
  /** The first key of a sequence that is waiting for its second key. */
  prefix: string | null;
  /** Digits typed so far, or null when none were typed. */
  count: number | null;
}

export const EMPTY_KEY_STATE: KeyState = { prefix: null, count: null };

export type Resolution =
  /** More keys are needed before this resolves to anything. */
  | { kind: "pending"; state: KeyState }
  /** A complete binding. `count` is null when the reader typed no digits. */
  | { kind: "action"; action: Action; count: number | null; state: KeyState }
  /** The keystroke means nothing here; the container lets it through. */
  | { kind: "ignored"; state: KeyState };

/** Larger counts are not reached in practice and risk a runaway loop. */
const MAX_COUNT = 9999;

function digitValue(token: string): number | null {
  return token.length === 1 && token >= "0" && token <= "9"
    ? token.charCodeAt(0) - 48
    : null;
}

function sameSequence(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((key, i) => key === b[i]);
}

/**
 * Resolve one token against the pending state.
 *
 * Grammar: `<digits> <key> <key>?` — a count applies to whatever the sequence
 * resolves to, and a prefix that goes nowhere is dropped without eating the
 * key that completed it (`g` then `j` still moves down).
 */
export function resolve(state: KeyState, token: string): Resolution {
  const digits = digitValue(token);
  if (digits !== null) {
    // A leading zero would mean its own motion in Vim; nothing is bound to it
    // here, so it starts no count and no sequence.
    if (state.count === null && digits === 0) {
      return { kind: "ignored", state: EMPTY_KEY_STATE };
    }
    const count = Math.min((state.count ?? 0) * 10 + digits, MAX_COUNT);
    return { kind: "pending", state: { prefix: null, count } };
  }

  const sequence = state.prefix === null ? [token] : [state.prefix, token];
  const binding = BINDINGS.find((b) => sameSequence(b.keys, sequence));
  if (binding) {
    return {
      kind: "action",
      action: binding.action,
      count: state.count,
      state: EMPTY_KEY_STATE,
    };
  }

  if (state.prefix !== null) {
    // The sequence died here. Drop the prefix and let the key stand on its
    // own: `gj` moves down, it does not swallow the `j`.
    return resolve({ prefix: null, count: state.count }, token);
  }

  if (PREFIXES.has(token)) {
    return { kind: "pending", state: { prefix: token, count: state.count } };
  }

  return { kind: "ignored", state: EMPTY_KEY_STATE };
}

/** The token a keystroke contributes, or null when nothing is bound to it. */
export function tokenOf(stroke: Keystroke): string | null {
  if (stroke.ctrl) {
    // Modifier combinations are named, so `ctrl+d` and `d` stay distinct.
    return stroke.alt || stroke.meta
      ? null
      : `ctrl+${stroke.key.toLowerCase()}`;
  }
  if (stroke.alt || stroke.meta) return null;
  // Named keys ("Escape", "ArrowDown", "F5") have no binding; every binding
  // is a single printable character.
  return stroke.key.length === 1 ? stroke.key : null;
}

/**
 * Whether the container must let a keystroke through untouched: anything typed
 * into a text field, anything pressed while a dialog, menu, or the help overlay
 * is open, and anything carrying a modifier the key map does not own.
 */
export function isSuppressed(
  stroke: Keystroke,
  context: { blocked: boolean; editable: boolean },
): boolean {
  if (context.blocked || context.editable) return true;
  if (!stroke.ctrl && !stroke.alt && !stroke.meta) return false;
  return !ownsModifier(stroke);
}

/** The only modifier combinations the key map consumes. */
function ownsModifier(stroke: Keystroke): boolean {
  if (!stroke.ctrl || stroke.alt || stroke.meta) return false;
  const key = stroke.key.toLowerCase();
  return key === "d" || key === "u";
}

/** Reduce a DOM keystroke. Kept out of the resolver so the resolver stays
 *  testable without a DOM. */
export function toKeystroke(event: KeyboardEvent): Keystroke {
  return {
    key: event.key,
    ctrl: event.ctrlKey,
    alt: event.altKey,
    meta: event.metaKey,
    shift: event.shiftKey,
  };
}

/** Whether a keystroke's target is a text field, where it must be typed
 *  instead of interpreted. */
export function isEditableTarget(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element || typeof element.tagName !== "string") return false;
  const tag = element.tagName.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  return element.isContentEditable === true;
}

/** The bindings for one overlay section, in table order. */
export function bindingsIn(section: Section): Binding[] {
  return BINDINGS.filter((b) => b.section === section);
}
