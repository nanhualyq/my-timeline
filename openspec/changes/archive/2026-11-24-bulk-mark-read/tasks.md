## 1. Backend: bulk write paths

- [x] 1.1 Add `db::mark_read_many(conn, ids)` that sets `is_read = 1` for the given ids, running in a transaction and chunking ids at 500 per statement; verify with tests that only the listed ids change, the returned count equals the rows actually changed, ids that were already read are not counted, and a list larger than one chunk (e.g. 1,200 ids) is fully applied.
- [x] 1.2 Add `db::mark_scope_read(conn, scope)` that reuses `filter_conditions("unread", scope)` in a single `UPDATE entries SET is_read = 1 WHERE ...`; verify with tests that the `all`, `feed`, and `folder` scopes each leave read state untouched outside the scope and that the returned count equals `count_entries(conn, "unread", scope)` for the same scope.
- [x] 1.3 Expose both functions as Tauri commands in `lib.rs` (named `mark_read_many` and `mark_scope_read`, neither taking an `is_read` flag) and register them in the handler list; verify with `cargo test` in `src-tauri` and a `cargo check` that reports no warnings for the new commands.

## 2. Frontend API surface

- [x] 2.1 Add `markReadMany(ids)` and `markScopeRead(scope)` wrappers to `src/api.ts` returning the changed count; verify `npm run check` passes.
- [x] 2.2 Add both commands to `src/__tests__/fakeBackend.ts`, mirroring the Rust semantics (id-list write and scope write, both returning a count); verify `npm test` still passes with no changes to existing specs.

## 3. Entry list affordances

- [x] 3.1 Add the header batch action to `EntryList.vue`, labelled with the number of rendered unread entries, disabled when that number is zero, and rendered only when the list is a triage list (feed, folder, or the unread view of all entries); verify in `entryList.spec.ts` that the label states the rendered unread count and that the action is absent in the unfiltered all-entries and starred views.
- [x] 3.2 Add the per-row batch action to `EntryList.vue`, revealed on row hover and focus, overlaying the row's top-right corner so it reserves no column, rendered only when at least one rendered entry above the row is unread, labelled with that count, and emitting the row's index; verify in `entryList.spec.ts` that it is absent on a row with nothing unread above and that its label matches the unread entries above it.
- [x] 3.3 Verify the two new affordances do not disturb the fixed row height or the star control: run `entryList.spec.ts` and confirm existing row-detail assertions pass unchanged.

## 4. Sidebar affordance

- [x] 4.1 Replace the module-level `feedMenuItems` / `folderMenuItems` in `Sidebar.vue` with per-row item lists that include "Mark all as read" with the row's unread count for feeds and folders, and omit it when that count is zero; verify with a sidebar spec that the item appears with its count and is absent at zero.
- [x] 4.2 Wire the sidebar item to `markScopeRead` through the existing `runAction` + `emit("changed")` path so failures surface in the sidebar's error line; verify with a spec that a failing call renders the error and a successful one emits `changed`.

## 5. Application wiring

- [x] 5.1 Add a handler in `App.vue` for marking the rendered page: build ids from the unread entries in `entries`, call `markReadMany`, then apply the view-dependent restore (unread view: `startOver()`; all view: patch the marked ids in place), and call `refreshCounts()`; verify with an app spec that the unread view refills with the next entries and the all view keeps the same rows in the same order.
- [x] 5.2 Add a handler for the row action that builds ids from `entries.slice(0, idx)` filtered to unread and shares the same restore path; verify with an app spec that entries below the row, including the row itself, stay unread.
- [x] 5.3 Add a handler for the sidebar whole-subscription mark that calls `markScopeRead(scope)` and restores through the reload path (`startOver()` on the unread view, `fetchWindow(depth)` otherwise); verify with an app spec that entries inside the feed which were never loaded also become read.
- [x] 5.4 Patch `selectedEntry` when the open entry is among the marked ids or belongs to a marked scope, so the reading pane's toggle shows it as read; verify with an app spec that the toggle offers "mark unread" after the batch mark.

## 6. Integration checks

- [x] 6.1 Verify the full suite passes: `npm test` and `cargo test` in `src-tauri`.
- [x] 6.2 Verify the type surface and build: `npm run check` and `npm run build`.
- [x] 6.3 Verify by hand in `npm run dev` that the header action marks only the rendered page, the row action leaves its own row unread, the sidebar action clears a whole feed while the Starred view is showing, and each label's number matches what changed.
