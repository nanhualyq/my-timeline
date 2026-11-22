## 1. Backend counts

- [x] 1.1 Add an `entry_counts` query in `src-tauri/src/db.rs` returning total and starred entry counts from a single `SELECT COUNT(*), COALESCE(SUM(is_starred), 0) FROM entries`; verify with `cargo test` covering the zero-entry case, counts after inserts, counts after `set_starred`, and that `set_read` changes neither count
- [x] 1.2 Expose it as an `entry_counts` Tauri command in `src-tauri/src/lib.rs` and register it in `generate_handler!`; verify `cargo build` compiles with the new command registered

## 2. Frontend counts

- [x] 2.1 Add the `EntryCounts` type and `entryCounts()` wrapper to `src/api.ts`; verify `npm run check` passes
- [x] 2.2 Hold both counts in `App.vue`, fetch them inside the existing `refreshCounts()` (in parallel with `listFeeds`/`listFolders`), and make `toggleStar()` await `refreshCounts()`; verify `npm run check` and that starring/unstarring from any view updates the Starred badge without losing the current list
- [x] 2.3 Render the two global badges in `Sidebar.vue` — total stored entries on "All entries", total starred entries on "★ Starred" — reusing the existing `.unread-badge` style and hiding them at zero; verify visually that 40 stored / 3 starred entries show badges `40` and `3`, and unstarring the last star removes the Starred badge

## 3. Starred view forces All

- [x] 3.1 In `App.vue.reload()`, compute the effective view as: starred selection → `"all"`, other global selections → `view`, feed/folder selections → `"all"` (unchanged), and pass the effective view as the active tab; verify `npm run check` and that selecting Starred while on the Unread tab lists every starred entry with the All tab active
- [x] 3.2 Add an `unreadFilterAvailable` prop to `EntryList.vue` (false only when the Starred view is selected) that hides the Unread tab, and guard `setView()` against `unread` while starred; verify visually that the Unread tab is absent in the Starred view, returns on "All entries", and that "All entries" still shows its previous tab after visiting Starred

## 4. Integration verification

- [x] 4.1 Run the full verification suite (`cargo test`, `npm run check`, `npm run build`); verify all pass
- [x] 4.2 Manual pass over the spec scenarios: badges count/hide/update on every mutation path (refresh, read toggle, star/unstar, feed removal), Starred always lists all starred entries with no Unread tab, the "All entries" filter survives a Starred visit, and feed/folder rows and badges behave exactly as before
