/**
 * Unwinding overlays with the system back action.
 *
 * The Android system back button is mirrored by the WebView's history, so each
 * open overlay — a dialog, the drawer, the reading screen — is paired with one
 * pushed history entry. Popping that entry asks the app to close one layer;
 * closing a layer by any other means (a close button, the backdrop) asks the
 * stack to drop the entry it no longer needs.
 *
 * The app owns the layers; this module only mirrors their depth. It never
 * decides what closing a layer means, so it can be tested without a DOM.
 */

export interface HistoryLike {
  pushState(state: unknown, title: string): void;
  go(delta: number): void;
}

export interface PopTargetLike {
  addEventListener(type: "popstate", listener: () => void): void;
  removeEventListener(type: "popstate", listener: () => void): void;
}

export interface BackStack {
  /**
   * Reconcile the mirrored history with the number of open layers. Call it
   * whenever the number changes; calling it with the current depth is a no-op.
   */
  sync(depth: number): void;
  /** The depth the stack believes is open. */
  depth(): number;
  dispose(): void;
}

export function createBackStack(options: {
  /** Called when the system back action reaches the app with a layer open. */
  onBack: () => void;
  history?: HistoryLike;
  target?: PopTargetLike;
}): BackStack {
  const history =
    options.history ??
    (typeof window === "undefined" ? null : (window.history as HistoryLike));
  const target = options.target ?? (typeof window === "undefined" ? null : window);

  let depth = 0;
  /** History pops this module caused itself, which must not close a layer. */
  let expected = 0;
  /** Set while a back action is closing a layer, so the resulting `sync` does
   *  not try to undo a history move that already happened. */
  let unwinding = false;

  function onPopState() {
    if (expected > 0) {
      expected -= 1;
      return;
    }
    if (depth === 0) return;
    // The history already moved back; closing the layer updates the depth.
    unwinding = true;
    options.onBack();
  }

  target?.addEventListener("popstate", onPopState);

  return {
    sync(next: number) {
      const want = Math.max(0, Math.trunc(next));
      if (want === depth) {
        unwinding = false;
        return;
      }

      if (want > depth) {
        for (let i = depth; i < want; i += 1) {
          history?.pushState({ backLayer: i + 1 }, "");
        }
        depth = want;
        unwinding = false;
        return;
      }

      const drop = depth - want;
      depth = want;
      if (unwinding) {
        // The pop that closed the layer is the one that shrank it.
        unwinding = false;
        return;
      }
      // Closed by a control rather than by the system: drop our own entries.
      expected += drop;
      history?.go(-drop);
    },

    depth() {
      return depth;
    },

    dispose() {
      target?.removeEventListener("popstate", onPopState);
    },
  };
}
