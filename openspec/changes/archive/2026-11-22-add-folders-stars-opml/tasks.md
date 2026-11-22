## 1. Schema and folder backend

- [x] 1.1 Add `folders` table, `feeds.folder_id` and `entries.is_starred` columns (tolerating repeated ALTER on open) plus the starred index in `db.rs`; verify existing `cargo test` suite still passes and a fresh + pre-existing DB both open cleanly
- [x] 1.2 Add folder CRUD functions (`create_folder`, `rename_folder`, `delete_folder` reassigning feeds to uncategorized, `reorder_folders`, `move_feed_to_folder`) with `FolderDto`; verify with `cargo test` covering delete-reassigns-to-NULL and position ordering
- [x] 1.3 Add `list_folders` returning folders by position with nested feeds and aggregated unread counts; verify with `cargo test` that counts and uncategorized feeds are correct

## 2. Entry listing scope and starring backend

- [x] 2.1 Replace `list_entries`'s `feed_id` parameter with a `scope` union (`all` / `feed` / `folder` / `starred`) and implement folder and starred filtering; verify with `cargo test` covering all four scopes combined with the `all`/`unread` views
- [x] 2.2 Add `set_starred(entry_id, is_starred)` mirroring `set_read`; verify with `cargo test` that starring does not affect `is_read` and persists in the DB
- [x] 2.3 Register the new commands (`create_folder`, `rename_folder`, `delete_folder`, `reorder_folders`, `move_feed_to_folder`, `list_folders`, `set_starred`) in `lib.rs`; verify with `cargo build` that the app compiles

## 3. Frontend selection model and star UI

- [x] 3.1 Update `src/api.ts` types and wrappers for the scope union, folder DTOs, and `setStarred`; verify `npm run check` passes
- [x] 3.2 Change `App.vue` selection to a discriminated union (`all`/`starred`/`folder`/`feed`) driving the new `list_entries` scope; verify `npm run check` and that switching between all four selections lists the expected entries
- [x] 3.3 Add a star toggle to `EntryList.vue` with persisted star state; verify starring an entry and re-selecting the view keeps it starred
- [x] 3.4 Add the global Starred view (sidebar node + unread-filter support) that never combines with feed/folder selection; verify starred entries from multiple feeds appear newest first and selecting a feed leaves the view

## 4. Sidebar folder tree

- [x] 4.1 Render folders between the global views and feeds in `Sidebar.vue`, with per-folder unread badges and the uncategorized group; verify visually that the structure matches the spec scenario "Sidebar structure"
- [x] 4.2 Add folder actions (create, rename, delete with confirm, reorder) and feed "move to folder" affordance; verify each action updates the sidebar and survives restart, and deleting a folder leaves its feeds in uncategorized

## 5. OPML interchange

- [x] 5.1 Add the `tauri-plugin-dialog` dependency (Rust and JS sides); verify `cargo build` and `npm run check` succeed
- [x] 5.2 Implement OPML parsing in a new `opml.rs` module (nested folders flattened to top-level ancestor, duplicate URLs skipped, malformed XML rejected); verify with `cargo test` covering fresh import, duplicate-URL skip preserving folder assignment, nested-folder flattening, and invalid XML
- [x] 5.3 Implement OPML generation (folder outlines wrap their feeds, uncategorized feeds exported top-level, XML-escaped); verify with `cargo test` including an export→import round-trip test on an empty state
- [x] 5.4 Add `import_opml` / `export_opml` Tauri commands (runtime checks deferred to task 6.2) using the dialog plugin for file selection, returning added/skipped/folder counts; verify manually: import reports counts, no feeds are fetched, and export writes a valid file

## 6. Integration verification

- [x] 6.1 Run the full verification suite (`cargo test`, `npm run check`, `npm run build`); verify all pass
- [x] 6.2 Manual end-to-end pass against spec scenarios: existing DB opens with feeds in uncategorized, folder lifecycle, starred view independence, OPML import→refresh→export round-trip; verify each scenario behaves as specified
