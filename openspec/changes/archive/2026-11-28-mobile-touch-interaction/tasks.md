## 1. Interaction axis

- [x] 1.1 Add `COARSE_MEDIA_QUERY`, a `COARSE_MAX_WIDTH = 1024` narrow-for-coarse threshold, and `inputMode` (`touch` | `pointer`) handling plus a `data-input` attribute to `src/lib/viewport.ts`, keeping `layoutOf`/`watchLayout`/`useLayout` semantics for `data-layout`; verify `viewport.spec.ts` covers all four rows of the table in design.md Decision 1
- [x] 1.2 Expose the interaction mode to components in `src/App.vue` alongside the existing `narrow` computed, and pass it down where row affordances live; verify with a component test that a wide coarse viewport yields `layout=wide` and `input=touch`
- [x] 1.3 Move the global touch-target selectors in `src/App.vue` from `:root[data-layout="narrow"]` to `:root[data-input="touch"]` and confirm the desktop window is unaffected; verify `touchTargets.spec.ts` and `npm run check` pass
- [x] 1.4 Update `src/__tests__/narrowLayout.spec.ts` and `src/__tests__/touchTargets.spec.ts` for the split axes, including a landscape-phone case (coarse, 915x412) asserting one pane and a landscape-tablet case asserting three columns; verify the suite passes

## 2. Gesture state machine

- [x] 2.1 Create `src/lib/gestures.ts` with `DRAG_SLOP`, `DIRECTION_LOCK`, `COMMIT_RATIO`, `COMMIT_MIN`, `EDGE_GUTTER`, `LONG_PRESS_MS`, and the pure `resolveDrag` state machine; verify new `src/__tests__/gestures.spec.ts` covers tap, vertical scroll, diagonal-preferring-vertical, lock, commit-left, commit-right, sub-threshold cancel, direction reversal, and the edge-gutter refusal
- [x] 2.2 Add the `useHorizontalDrag` composable in the same module using passive Pointer Events and a one-tick `suppressClick` flag; verify with a component test that a commit emits once, that a tap emits nothing, and that no listener throws when Pointer Events are unavailable

## 3. Undo for batch marks

- [x] 3.1 Add single-slot `pendingUndo` state and `UNDO_WINDOW_MS = 3000` to `src/App.vue`, armed from `applyBatchRead` so both `markRenderedRead` and `markAboveRead` record the ids they mark; verify a test asserting undo is offered after each of the two existing activations
- [x] 3.2 Implement undo execution — `setRead(id, false)` per recorded id, `patchEntry`, then `refreshCounts` — and reload the list when it is filtered to unread so restored entries are visible; verify tests for exact restoration, for an entry the batch did not mark being left alone, and for the control withdrawing when the window passes
- [x] 3.3 Create the undo bar component (count-bearing label, touch-sized control, clear of the bottom safe area on Android) and mount it from `src/App.vue`; verify a component test that it renders the count and emits the undo intent
- [x] 3.4 Confirm a second batch replaces a pending undo instead of stacking; verify with a test that only the newer batch's ids are restored

## 4. Row gestures, starred marker, and long-press menu

- [x] 4.1 Restructure the row markup in `src/components/EntryList.vue` into a surface plus leading/trailing swipe panels driven by a `--drag-x` custom property, with `touch-action: pan-y` on the row; verify a component test that the panel matching the drag direction is the visible one
- [x] 4.2 Wire `useHorizontalDrag` on touch rows: left commits `toggleStar`, right commits `markAbove(index)`, with the revealed label carrying the count for the rightward case; verify tests for both commits, for `suppressClick` preventing the entry from opening, and for the commit threshold
- [x] 4.3 Remove the `☆` star button and the `↑N` chip from touch rows, drop the compensating `.narrow-row .entry-body` padding, and keep both controls on the pointer layout only; verify `entryList.spec.ts` asserts the star control is absent for a touch row and the row body has no reserved trailing space
- [x] 4.4 Render the starred state as a non-interactive marker in the meta line exactly when the row has no star button, and not alongside it; verify tests for a starred touch row and a starred pointer row
- [x] 4.5 Create the long-press menu component reusing `RowMenu`'s item shape and `blockKeyboard()`, register it in `openLayers` so system back closes it, and open it from a 500 ms long press on a touch row; verify tests that both items are offered and apply, that dismissal changes nothing, and that back closes it before anything under it

## 5. Reading pane

- [x] 5.1 Add the star control to `src/components/ReadingPane.vue` on every layout with `aria-label` state and a touch-sized target, emitted to `src/App.vue` and routed through the existing `setStarred` path; verify tests for starring, unstarring, the list row and starred count following, and the read state staying unchanged
- [x] 5.2 Wire `useHorizontalDrag` on the reading pane content: left emits `next`, right emits `previous`, sized with a doubled edge gutter, leaving vertical scrolling and text selection intact; verify tests that a vertical drag scrolls without switching and that a horizontal drag emits the right direction
- [x] 5.3 Handle `previous`/`next` in `src/App.vue` as `stepCursor(-1)`/`stepCursor(1)` and verify the existing behaviours still follow a swipe: the entry is marked read, swiping past the last loaded row loads the next batch, and swiping at the end keeps the open entry
- [x] 5.4 Add the reading screen's indication that neighbouring entries exist, in the form judged best on a rendered phone screen, disabled when no neighbour exists; verify a test that it reflects availability at both ends

## 6. Returning from the reader

- [x] 6.1 Add `revealEntry(id)` to `src/components/EntryList.vue` and call it post-flush from `src/App.vue` when the reader screen closes, replacing the `v-show`-ineffective watcher path; verify a test that closing the reader after reading several entries leaves the last read entry selected and revealed
- [x] 6.2 Confirm the jsdom `scrollIntoView` guard still holds on the new path; verify the narrow-layout suite runs without a scroll error

## 7. Verification

- [x] 7.1 Run `npm test` and confirm every suite passes, including the new `gestures.spec.ts` and the updated narrow-layout, touch-target, entry-list, and viewport suites
- [x] 7.2 Run `npm run check` and confirm `vue-tsc` reports no errors across the changed components and the new modules
- [x] 7.3 Run `openspec validate mobile-touch-interaction --strict` and confirm the change validates after implementation
- [x] 7.4 Manually exercise the touch path in the Android build: drag a row each way, long-press a row, swipe the article both ways past the end of the loaded list, undo a batch mark, and the system back action from the list, the reader, and the long-press menu
