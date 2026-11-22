## Why

Manual folder reordering adds unnecessary UI clutter (move up/down buttons), an extra Tauri command, and a `position` column that must be kept in sync for little benefit. Users expect folders to appear in plain alphabetical order, so the app should sort folders by their name using the default string comparison and drop the reordering feature entirely.

## What Changes

- Folders are always displayed in default string comparison order of their names (byte/code-unit order, case-sensitive), with no user-controllable order.
- **BREAKING**: Remove the "Move folder up" / "Move folder down" controls from the sidebar folder rows.
- **BREAKING**: Remove the `reorder_folders` Tauri command, the `db::reorder_folders` function, and the `reorderFolders` client API.
- Remove the `position` column from the `folders` schema and stop writing/reading it (no migration handling for existing databases, per decision).
- Update `list_folders` ordering from `ORDER BY position, id` to default string comparison on `name`.
- Update the feed-folders spec: remove the folder reordering requirement and add a requirement that folders are ordered by name.

## Capabilities

### New Capabilities

- None

### Modified Capabilities

- `feed-folders`: Replace the "User can reorder folders" requirement with an ordering-by-name requirement; folders have no user-controlled order.

## Impact

- Backend: `src-tauri/src/db.rs` (folders schema, `create_folder`, `list_folders`, remove `reorder_folders`), `src-tauri/src/lib.rs` (remove command + handler).
- Frontend: `src/api.ts` (remove `reorderFolders`), `src/components/Sidebar.vue` (remove move up/down buttons and `moveFolder`).
- Tests: `db.rs` unit tests covering `position` and `reorder_folders` must be updated or removed; add coverage for name ordering.
- Existing local databases: the `position` column may remain in already-created tables but is no longer referenced by any code.
