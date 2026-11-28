## Why

The narrow layout renders the same keyboard-and-pointer interface squeezed into one column: entry rows carry two permanent buttons (`☆` star and an `↑N` batch chip) that exist only because no hover is available to reveal them, and the row body even reserves horizontal padding for that chip. On a phone that costs title width and first-screen density for controls a fingertip reaches better by swiping. The one interaction a phone actually affords — horizontal drag — is unused everywhere in the application.

The current split also keys everything off viewport width alone, so a landscape phone (~915 × 412 CSS pixels) receives the three-column desktop layout whose hover-revealed batch action can never be revealed by a fingertip.

## What Changes

- Introduce a **touch interaction mode** decided by narrow width **or** a coarse primary pointer, separate from the width-based decision that chooses one-pane versus three-column *structure*.
- Remove the `☆` star button and the `↑N` batch-mark chip from entry rows on touch layouts, returning the reserved width to the title and excerpt.
- Add **horizontal row gestures**: drag the row left to toggle the star, drag the row right to mark the rendered entries above it read.
- Show the starred state of a row as a static marker once the row's star button is gone, so state stays readable.
- Add a **long-press row menu** offering the same two actions, so they remain reachable where a horizontal drag is unavailable or is consumed by an assistive technology.
- Add an **undo bar** after any batch mark read, covering both the gesture and the existing header action. **BREAKING** for the current `bulk-mark-read` rule that batch marking is one-way and offers no undo.
- Add **horizontal swipes in the reading pane** to move to the previous and next entry, reusing the existing cursor movement so paging past the loaded rows keeps working.
- Add a **star control to the reading pane**, since a reader inside an article otherwise has to return to the list to change it.
- Change returning from the reading screen to land the list on the entry that is open, instead of restoring the previous scroll position.
- Gestures reserve a gutter at the screen edges for the Android system back gesture, lock their direction before acting, and leave tap-to-open unchanged.

## Capabilities

### New Capabilities

- `touch-gestures`: the horizontal gesture vocabulary of the touch layout — which drag does what on a row and in the reading pane, and the mechanics that keep those drags from colliding with scrolling, tapping, system back, and assistive technology.

### Modified Capabilities

- `entry-row-presentation`: a row no longer keeps a star control; the starred state becomes a static marker and the actions are reached by gesture or menu.
- `mobile-shell`: touch reachability is satisfied by gesture, long-press menu, or visible control rather than by a permanently visible row control; the reading screen gains a star action; returning to the list follows the open entry rather than restoring its scroll position.
- `bulk-mark-read`: batch marking stays immediate but becomes reversible for a short window, replacing the one-way/no-undo rule.
- `entry-starring`: the reading pane shows and toggles the open entry's star.

## Impact

- Frontend: `src/lib/viewport.ts` (pointer-capability query, structure vs interaction split), a new gesture module, `EntryList.vue`, `ReadingPane.vue`, `App.vue`, and a new undo bar component.
- Tests: `viewport.spec.ts`, `narrowLayout.spec.ts`, `touchTargets.spec.ts`, `entryList.spec.ts`, plus new gesture and undo coverage.
- No backend, Tauri command, storage, or sync format changes. No new runtime dependency.
