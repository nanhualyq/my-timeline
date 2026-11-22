## Why

Sidebar rows are crowded: each folder/feed row renders its actions (refresh, move-to-folder select, remove, rename, delete) as inline controls that squeeze the name into a narrow ellipsized span. Users also have no way to refresh a whole folder at once, and no proper place to edit a feed's title or move it between folders.

## What Changes

- **BREAKING**: Folder and feed row actions move out of the row into a per-row dropdown menu (⋮); a row shows only its name, unread badge, and the menu control. The inline refresh button, move-to-folder select, remove button, and folder rename/delete buttons disappear from the rows.
- Folder menu: Refresh folder, Rename, Delete. Feed menu: Edit…, Refresh, Remove.
- New folder refresh: refreshes every feed under the folder with the same bounded concurrency as refresh-all; reports one summary alert ("X feed(s) refreshed, Y failed") when done; per-feed failures keep their error dots.
- New feed edit dialog containing: folder assignment (dropdown, replaces the inline select), a custom title input (persists across refreshes; clearing it reverts to the feed-provided title), and read-only status info (URL, last fetched time, last fetch error). Remove stays in the row menu, not the dialog.
- Backend: new `refresh_folder` command returning per-feed outcomes; new `custom_title` column on `feeds` with display-title resolution (`COALESCE`); new command to set/clear a feed's custom title.
- Rows keep all existing behaviors otherwise (selection, unread badges, error dots, confirmations).

## Capabilities

### New Capabilities

- None

### Modified Capabilities

- `feed-folders`: Folder row actions are exposed through a row menu instead of inline buttons; a new requirement lets the user refresh all feeds of a folder with a summary result.
- `rss-reader`: Feed row actions are exposed through a row menu instead of inline controls; a new requirement covers the feed edit dialog (folder assignment, persistent custom title, read-only status).

## Impact

- Frontend: `src/components/Sidebar.vue` (row menus, remove inline controls), new feed edit dialog component, `src/App.vue` (wiring for folder refresh + edit dialog), `src/api.ts` (new `refresh_folder`, `set_feed_title` calls).
- Backend: `src-tauri/src/db.rs` (`custom_title` column migration, display-title resolution in feed queries, set/clear title function), `src-tauri/src/reader.rs` (`refresh_folder` reusing the bounded-concurrency refresh), `src-tauri/src/lib.rs` (new commands), `src-tauri/src/opml.rs` (export uses display title).
- Tests: db tests for custom title persistence/resolution, reader tests for folder refresh outcomes, existing tests that assume inline controls (none in Rust; frontend verified via `vue-tsc` and the dev server).
