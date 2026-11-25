## 1. Key Map Foundation

- [x] 1.1 Create `src/lib/keymap.ts` exporting a binding table where each entry carries its key sequence, the action it resolves to, and the description shown in the help overlay, and verify every binding listed in `specs/keyboard-navigation/spec.md` appears exactly once in the table
- [x] 1.2 Implement the pure resolver (pending prefix, pending count, and incoming keystroke in; `pending` / `action` / `ignored` out, with no timers and no DOM access) and verify it with unit tests in `src/__tests__/keymap.spec.ts` covering single keys, `gg`/`ga`/`gs`, an unknown `g` suffix, a count before a motion, a count before `G`, and a count before `g`
- [x] 1.3 Add the prefix-discard rule to the resolver's contract (a pending prefix is reportable as expired) and verify a unit test shows an expired `g` leaves state unchanged and does not act as a bare `g`
- [x] 1.4 Verify the count-consumption rules with unit tests: a resolved motion consumes the count, an absolute jump to the top ignores it (`3gg` → first row), and a resolved action clears it

## 2. Key Gating

- [x] 2.1 Add `src/lib/keyboard-blocks.ts` with `blockKeyboard()`, `unblockKeyboard()`, and a reactive `keyboardBlocked` getter, and verify a unit test shows the getter stays true until every registered blocker releases
- [x] 2.2 Register `SettingsDialog.vue`, `FeedEditDialog.vue`, and the open state of `RowMenu.vue` as blockers without removing their existing `Escape` listeners, and verify the existing dialog and row-menu tests still pass
- [x] 2.3 Add the container-side gating checks (reactive blocked state, an input/textarea/select/contenteditable event target, and any `Ctrl`/`Alt`/`Meta` keystroke other than `Ctrl-d`/`Ctrl-u`) and verify a unit test asserts each of those states resolves every binding to `ignored`

## 3. The Cursor

- [x] 3.1 Install the single `document` keydown listener in `App.vue`, feed it through the resolver, and dispatch resolved actions, and verify the app mounts and runs with the listener attached and no duplicate-listener warnings in the test run
- [x] 3.2 Derive the cursor from `selectedEntryId` (with `lastCursorIndex` maintained only as the clamp fallback), wire `j`/`k` to move it through `selectEntry`, and verify integration tests show `j` on a fresh list opens and marks the first row read, and `k` on that row leaves it read
- [x] 3.3 Clamp movement at both ends of the loaded rows and verify integration tests show `k` on the first row and `j` on the last row with no further entries leave the cursor and the network calls unchanged
- [x] 3.4 Make `j`/`k` no-ops on an empty list and verify an integration test shows no entry is opened and no request is made
- [x] 3.5 Set the cursor from a mouse click so keyboard movement continues from the clicked row, and verify an integration test clicks a row, presses `j`, and lands on the following row
- [x] 3.6 Implement `gg`/`G` and numeric counts for movement, and verify integration tests cover `5j` from row 5 landing on row 10, `20j` on a ten-row list stopping at row 10, and `12G` landing on the last row when fewer are loaded
- [x] 3.7 Implement list scrolling for the cursor with `scrollIntoView({ block: "nearest" })`, verify the cursor row is scrolled into view after a `G` jump in a jsdom integration test with stubbed geometry
- [x] 3.8 Implement `Ctrl-d`/`Ctrl-u` half-page movement using a lazily measured visible-row count (no hardcoded row height), call `preventDefault`, and verify an integration test shows the cursor moving by about half the visible rows and the webview default being suppressed

## 4. Actions

- [x] 4.1 Bind `m` to the cursor row's read toggle via the existing single-entry path and verify an integration test toggles read then unread and checks the sidebar counts follow
- [x] 4.2 Bind `s` to starring and verify an integration test stars the cursor row and shows it in the Starred view's list and badge
- [x] 4.3 Bind `o` to opening the cursor entry's link through the existing opener call and verify an integration test asserts the link is opened and the open entry is unchanged
- [x] 4.4 Bind `v` to the All/Unread tab switch through the existing `setView`, verify an integration test switches tabs and keeps the sidebar selection, and verify `v` does nothing in the Starred view
- [x] 4.5 Bind `A` to the existing rendered-rows batch action and verify an integration test marks exactly the rendered unread rows and that an unread list still refills
- [x] 4.6 Bind `r` to the existing refresh-all action and verify an integration test shows the refresh request and that the loaded rows are kept
- [x] 4.7 Make `m`, `s`, and `o` no-ops when no entry is open and verify an integration test shows no request is made

## 5. Reading Pane

- [x] 5.1 Bind `J`/`K` to scrolling the reading pane by one screen without moving the cursor and verify an integration test asserts only the pane's scroll position changes
- [x] 5.2 Reset the pane's scroll position when a different entry is opened, after the new content is rendered, and verify an integration test scrolls a pane, moves the cursor, and observes `scrollTop` back at 0

## 6. Burst Coalescing

- [x] 6.1 Add `applyReadSilently(ids)` beside `applyBatchRead` (mark many, patch rows, one count refresh, no `startOver`) and verify an integration test shows the rows keeping their positions and rendering as read in an unread list
- [x] 6.2 Accumulate passed row ids and the landed row into pending state on every move, arming and resetting a flush timer, and verify with fake timers that ten rapid moves produce one content fetch, one read-state write, and one count refresh
- [x] 6.3 Keep the rendered cursor and read indicator in step with the keystroke rather than with the flush, and verify an integration test asserts the row and its read state update before the flush resolves
- [x] 6.4 Force the flush before any list replacement (`select`, `setView`, `startOver`), on window blur, and on unmount, and verify integration tests show marks surviving an immediate feed switch and an immediate tab switch
- [x] 6.5 Verify the stale-response guard holds on the burst path with an integration test that resolves an earlier content request after the cursor has moved on and asserts the pane never shows the abandoned entry

## 7. Reloads and Paging

- [x] 7.1 Keep the cursor on its entry across `reload`, `refreshAll`, and `loadMore` by re-deriving position from the entry id, and verify integration tests refresh and load a batch with the cursor on row 20
- [x] 7.2 Stand in at the same index, or at the last row, when the cursor's entry disappears on a reload, keep clearing the cursor and the pane when the reader removes the open entry's feed, and verify integration tests cover both cases
- [x] 7.3 Clear the cursor when the selection changes and verify an integration test selects a different feed and finds no entry open
- [x] 7.4 Request the next batch when the cursor lands on the last loaded row with more matching, apply a pending advance onto the first row of the new batch, and verify integration tests cover continuing onto the new batch and stopping when the batch comes back empty
- [x] 7.5 Verify `G` lands on the last loaded row without loading the rest of the list, extends by exactly one batch per further press at that row, appends at most one batch however often the growing list re-renders, and stays put when nothing more matches

## 8. Help Overlay

- [x] 8.1 Add a help overlay component that renders the binding table (keys and descriptions) from `src/lib/keymap.ts` and opens on `?`, and verify an integration test opens it and finds a row for every binding in the table
- [x] 8.2 Close the overlay on `?` and `Escape`, register it as a keyboard blocker while open, and verify an integration test shows `j` and `r` doing nothing while it is open

## 9. Verification

- [x] 9.1 Run `npm run check` and verify no type errors
- [x] 9.2 Run `npm test` and verify the whole suite passes, including the new resolver and integration tests
- [x] 9.3 Verify the spec's scenarios not covered by a named test are either covered by an existing one or recorded as manual steps in the change, and list any gaps in the final summary
