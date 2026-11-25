import { describe, expect, it } from "vitest";
import {
  BINDINGS,
  EMPTY_KEY_STATE,
  SECTIONS,
  bindingsIn,
  isEditableTarget,
  isSuppressed,
  resolve,
  tokenOf,
  type Keystroke,
  type KeyState,
  type Resolution,
} from "../lib/keymap";
import { blockKeyboard, keyboardBlocked, resetKeyboardBlocks } from "../lib/keyboard-blocks";

/** Type a sequence of tokens, returning the resolution of the last one. */
function type(tokens: string[], from: KeyState = EMPTY_KEY_STATE): Resolution {
  let state = from;
  let last: Resolution = { kind: "ignored", state };
  for (const token of tokens) {
    last = resolve(state, token);
    state = last.state;
  }
  return last;
}

function actionOf(tokens: string[]) {
  const result = type(tokens);
  if (result.kind !== "action") {
    throw new Error(`expected an action, got ${result.kind}`);
  }
  return { action: result.action, count: result.count };
}

describe("the binding table", () => {
  it("binds every action exactly once", () => {
    const actions = BINDINGS.map((b) => b.action);
    expect(new Set(actions).size).toBe(actions.length);
  });

  it("never binds the same sequence twice", () => {
    const sequences = BINDINGS.map((b) => b.keys.join(" "));
    expect(new Set(sequences).size).toBe(sequences.length);
  });

  it("describes every binding for the overlay", () => {
    for (const binding of BINDINGS) {
      expect(binding.label, binding.action).toBeTruthy();
      expect(binding.description, binding.action).toBeTruthy();
      expect(SECTIONS).toContain(binding.section);
    }
    // Grouping must not lose a binding.
    const grouped = SECTIONS.flatMap((section) => bindingsIn(section));
    expect(grouped).toHaveLength(BINDINGS.length);
  });
});

describe("resolving single keys", () => {
  it.each([
    ["j", "cursor-down"],
    ["k", "cursor-up"],
    ["G", "cursor-bottom"],
    ["m", "toggle-read"],
    ["s", "toggle-star"],
    ["o", "open-link"],
    ["v", "toggle-view"],
    ["A", "mark-rendered"],
    ["r", "refresh-all"],
    ["J", "pane-scroll-down"],
    ["K", "pane-scroll-up"],
    ["?", "toggle-help"],
  ])("%s resolves to %s", (token, action) => {
    expect(actionOf([token])).toEqual({ action, count: null });
  });

  it("reports an unbindable character as ignored", () => {
    expect(type(["x"]).kind).toBe("ignored");
  });

  it("leaves no pending state behind an action", () => {
    expect(type(["j"]).state).toEqual(EMPTY_KEY_STATE);
  });
});

describe("the g prefix", () => {
  it("starts a sequence without acting", () => {
    const result = type(["g"]);
    expect(result.kind).toBe("pending");
    expect(result.state).toEqual({ prefix: "g", count: null });
  });

  it.each([
    [["g", "g"], "cursor-top"],
    [["g", "a"], "go-all"],
    [["g", "s"], "go-starred"],
  ])("%s resolves to %s", (tokens, action) => {
    expect(actionOf(tokens).action).toBe(action);
  });

  it("drops a prefix that leads nowhere and still honours the second key", () => {
    const result = type(["g", "j"]);
    expect(result.kind).toBe("action");
    expect(result).toMatchObject({ action: "cursor-down" });
    expect(result.state).toEqual(EMPTY_KEY_STATE);
  });

  it("treats a prefix followed by an unbound key as a dead end", () => {
    const result = type(["g", "x"]);
    expect(result.kind).toBe("ignored");
    expect(result.state).toEqual(EMPTY_KEY_STATE);
  });

  it("can start a sequence from inside another dead end", () => {
    // `g` then `g` then `g`: the first pair is a jump, the third starts over.
    const result = type(["g", "g", "g"]);
    expect(result.kind).toBe("pending");
    expect(result.state).toEqual({ prefix: "g", count: null });
  });
});

describe("the count prefix", () => {
  it("accumulates digits", () => {
    let state = EMPTY_KEY_STATE;
    for (const token of ["1", "2"]) {
      const result = resolve(state, token);
      state = result.state;
    }
    expect(state).toEqual({ prefix: null, count: 12 });
  });

  it("applies to the motion that follows", () => {
    expect(actionOf(["5", "j"])).toEqual({ action: "cursor-down", count: 5 });
  });

  it("applies to an absolute jump", () => {
    expect(actionOf(["1", "2", "G"])).toEqual({ action: "cursor-bottom", count: 12 });
  });

  it("is consumed by the action it applies to", () => {
    expect(type(["5", "j"]).state).toEqual(EMPTY_KEY_STATE);
    // A second motion is a single step again.
    expect(actionOf(["5", "j", "j"])).toEqual({ action: "cursor-down", count: null });
  });

  it("survives a prefix that leads nowhere", () => {
    expect(actionOf(["3", "g", "j"])).toEqual({ action: "cursor-down", count: 3 });
  });

  it("reaches the action behind a completed sequence", () => {
    expect(actionOf(["3", "g", "g"])).toEqual({ action: "cursor-top", count: 3 });
  });

  it("does not start on a leading zero", () => {
    const result = type(["0"]);
    expect(result.kind).toBe("ignored");
    expect(result.state).toEqual(EMPTY_KEY_STATE);
  });

  it("is capped rather than letting a stuck key build it forever", () => {
    let state: KeyState = EMPTY_KEY_STATE;
    for (let i = 0; i < 20; i++) state = resolve(state, "9").state;
    expect(state.count).toBe(9999);
  });
});

describe("deciding whether the container may use a key", () => {
  const plain = { key: "j" };

  it("uses a plain key when nothing is in the way", () => {
    expect(isSuppressed(plain, { blocked: false, editable: false })).toBe(false);
  });

  it("leaves every binding alone while a surface holds the keyboard", () => {
    for (const binding of BINDINGS) {
      expect(
        isSuppressed({ key: binding.keys[binding.keys.length - 1] }, { blocked: true, editable: false }),
        binding.label,
      ).toBe(true);
    }
  });

  it("leaves every binding alone while a text field has focus", () => {
    for (const binding of BINDINGS) {
      expect(
        isSuppressed({ key: binding.keys[binding.keys.length - 1] }, { blocked: false, editable: true }),
        binding.label,
      ).toBe(true);
    }
  });

  it.each(["ctrl+a", "ctrl+c", "ctrl+r", "ctrl+v", "alt+j", "meta+k"])(
    "does not consume %s",
    (combo) => {
      const [modifier, key] = combo.split("+");
      const stroke: Keystroke = {
        key,
        ctrl: modifier === "ctrl",
        alt: modifier === "alt",
        meta: modifier === "meta",
      };
      expect(isSuppressed(stroke, { blocked: false, editable: false })).toBe(true);
    },
  );

  it.each(["d", "u"])("owns ctrl+%s", (key) => {
    expect(
      isSuppressed({ key, ctrl: true }, { blocked: false, editable: false }),
    ).toBe(false);
  });

  it("keeps shift, which carries the uppercase bindings", () => {
    expect(isSuppressed({ key: "G", shift: true }, { blocked: false, editable: false })).toBe(
      false,
    );
  });
});

describe("reading keystrokes", () => {
  it("names modifier combinations so they stay distinct from the bare key", () => {
    expect(tokenOf({ key: "d", ctrl: true })).toBe("ctrl+d");
    expect(tokenOf({ key: "D", ctrl: true })).toBe("ctrl+d");
    expect(tokenOf({ key: "d" })).toBe("d");
  });

  it("has no token for named keys", () => {
    expect(tokenOf({ key: "Escape" })).toBeNull();
    expect(tokenOf({ key: "ArrowDown" })).toBeNull();
  });

  it("has no token for combinations with alt or meta", () => {
    expect(tokenOf({ key: "d", ctrl: true, alt: true })).toBeNull();
    expect(tokenOf({ key: "j", meta: true })).toBeNull();
  });

  it("recognises the fields that must keep their own keys", () => {
    const input = document.createElement("input");
    const textarea = document.createElement("textarea");
    const div = document.createElement("div");
    expect(isEditableTarget(input)).toBe(true);
    expect(isEditableTarget(textarea)).toBe(true);
    expect(isEditableTarget(div)).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
});

describe("the keyboard blocker counter", () => {
  it("stays blocked until every surface has released", () => {
    resetKeyboardBlocks();
    const first = blockKeyboard();
    const second = blockKeyboard();
    expect(keyboardBlocked.value).toBe(true);
    first();
    expect(keyboardBlocked.value).toBe(true);
    second();
    expect(keyboardBlocked.value).toBe(false);
  });

  it("ignores a release that has already happened", () => {
    resetKeyboardBlocks();
    const release = blockKeyboard();
    release();
    release();
    expect(keyboardBlocked.value).toBe(false);
  });
});
