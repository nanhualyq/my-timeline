import { computed, ref } from "vue";

/**
 * A shared count of surfaces that own the keyboard while they are open: the
 * dialogs, an open row menu, and the help overlay.
 *
 * A counter rather than a boolean because these overlap — a row menu can be
 * open while nothing else is, and two dialogs can be mounted in sequence
 * without an unmount in between. Each surface releases what it registered, so
 * the keyboard comes back exactly once, no matter the order.
 *
 * This is deliberately not a DOM query (`.row-menu-panel` in the document) and
 * not keymap state: the surfaces already know when they are open, and the
 * resolver should not know their markup.
 */
const blockers = ref(0);

/** Hold the keyboard. Returns the release function, so a caller cannot
 *  accidentally release twice or forget an identifier. */
export function blockKeyboard(): () => void {
  blockers.value += 1;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    blockers.value = Math.max(0, blockers.value - 1);
  };
}

/** True while any surface has claimed the keyboard. */
export const keyboardBlocked = computed(() => blockers.value > 0);

/** Test seam: forget every registration. */
export function resetKeyboardBlocks(): void {
  blockers.value = 0;
}
