## 1. Backend: library-wide unread count

- [x] 1.1 Extend `EntryCountsDto` / `entry_counts` in `db.rs` with `unread_entries` (`SUM(is_read = 0)` in the same statement) and verify with a test that the count tracks read and unread entries while `all_entries` and `starred_entries` keep their values.

## 2. Frontend API and test double

- [x] 2.1 Add `unread_entries` to the `EntryCounts` interface in `src/api.ts` and to the fake backend's `entry_counts`, mirroring the Rust aggregate; verify `npm run check` and `npm test` pass.

## 3. Sidebar

- [x] 3.1 Give the "All entries" row a `RowMenu` whose only item is the library-wide "Mark all as read (N)", hidden when N is zero, reusing the existing per-row menu builder; verify in `sidebar.spec.ts` that the item states the total and disappears at zero.
- [x] 3.2 Wire that item to `markScopeRead({ kind: "all" })` through `runAction()` + `emit("changed")`; verify in `sidebar.spec.ts` that the call carries the `all` scope and that a failing call renders the sidebar's error line.
- [x] 3.3 Verify the "Starred" row still has no batch action (no menu item and no trigger) in `sidebar.spec.ts`.

## 4. Application wiring

- [x] 4.1 Pass the library-wide unread count to `Sidebar` as a new prop and verify with an app spec that the label matches the number of unread entries the fake backend holds.
- [x] 4.2 Verify the global mark reaches entries the list never loaded and restores the view: an app spec that marks everything read from the "All entries" row while a single feed is selected, then asserts every stored entry is read, the unread list is empty, and the open entry is re-read as read.

## 5. Checks

- [x] 5.1 Verify the full suite passes: `npm test`, `cargo test` in `src-tauri`, `npm run check`, `npm run build`.
