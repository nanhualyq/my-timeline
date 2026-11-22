## 1. Backend (Rust)

- [x] 1.1 Remove `position` from the `folders` schema in `src-tauri/src/db.rs` (`FOLDERS_TABLE`), from `FolderDto`, and from `create_folder` (insert only `name`); verify `cargo test` in `src-tauri` passes with the schema/creation test updated to assert no `position` is stored
- [x] 1.2 Change `list_folders` to `ORDER BY f.name, f.id` (default string comparison) and update its doc comment; verify a unit test passes where folders created as "Tech" then "Books" are listed "Books" first
- [x] 1.3 Delete `db::reorder_folders` and its test assertions in `src-tauri/src/db.rs`; verify `cargo test` passes and `grep -rn "reorder\|position" src-tauri/src` returns no folder-related hits
- [x] 1.4 Remove the `reorder_folders` handler and its `generate_handler!` registration in `src-tauri/src/lib.rs`; verify `cargo build` and `cargo test` in `src-tauri` succeed

## 2. Frontend (Vue/TS)

- [x] 2.1 Remove `reorderFolders` and the `position` field from `src/api.ts`; verify `npm run check` (vue-tsc) passes
- [x] 2.2 Remove the `reorderFolders` import, `moveFolder` function, and the "Move folder up"/"Move folder down" buttons from `src/components/Sidebar.vue`; verify `npm run check` passes and `grep -rn "reorderFolders\|moveFolder" src` returns nothing
- [x] 2.3 Verify observable behavior with `npm run dev:web`: folder rows show no move up/down controls and folders appear in name order (e.g., "Banana" before "apple", "Books" before "Tech")

## 3. Tests and validation

- [x] 3.1 Add db unit tests in `src-tauri/src/db.rs` covering the delta spec scenarios: name order regardless of creation order, case-sensitive default comparison ("Banana" before "apple"), and position change after rename; verify `cargo test` passes
- [x] 3.2 Run `openspec validate sort-folders-by-name --strict` and verify it reports no errors
