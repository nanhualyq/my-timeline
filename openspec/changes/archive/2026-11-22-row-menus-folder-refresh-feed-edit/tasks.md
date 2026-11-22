## 1. Backend: persistent custom title

- [x] 1.1 Add the `custom_title` column migration and resolve display titles in `src-tauri/src/db.rs`: `COALESCE(custom_title, title) AS title` in `list_feeds` and `get_feed`, and order feeds by the display title; verify `cargo test` passes with a new test showing `get_feed`/`list_feeds` return the custom title while the stored site `title` stays untouched
- [x] 1.2 Add `db::set_feed_title(conn, feed_id, Option<&str>)` (set or clear) plus the `set_feed_title` Tauri command in `lib.rs` and an `api.ts` wrapper; verify `cargo test` covers set → shows custom, clear → reverts to feed-provided title, unknown feed rejected, and `cargo build` succeeds
- [x] 1.3 Confirm OPML export emits the display title (it reads `FeedDto.title`) and add a test that an exported feed with a custom title carries it; verify `cargo test` passes

## 2. Backend: folder refresh

- [x] 2.1 Extract `refresh_all`'s bounded-concurrency loop in `src-tauri/src/reader.rs` into a shared helper over a target list, and add `refresh_folder(conn, folder_id)` that refreshes only that folder's feeds (unknown folder → error); verify `cargo test` passes with a new test proving only the folder's feeds are fetched and an unknown folder id fails
- [x] 2.2 Expose `refresh_folder` as a Tauri command returning `Vec<RefreshOutcome>` (same shape as `refresh_all`) and add the `refreshFolder` wrapper in `api.ts`; verify `cargo build` and `cargo test` succeed

## 3. Frontend: row menus

- [x] 3.1 Create `src/components/RowMenu.vue`: a ⋮ trigger button with an absolutely positioned dropdown panel that closes on outside click, Escape, and after an item is chosen; verify `npm run check` passes
- [x] 3.2 Rework the folder row in `Sidebar.vue` to show only name, unread badge, and the menu (items: Refresh folder, Rename, Delete), removing the inline rename/delete buttons; verify `npm run check` passes and `grep` finds no inline folder action buttons
- [x] 3.3 Rework the feed row in `Sidebar.vue` to show only title, unread badge, and the menu (items: Edit…, Refresh, Remove), removing the inline refresh button, move-to-folder select, and remove button; verify `npm run check` passes and `grep` finds no `move-select`/inline refresh/remove controls left in the feed row

## 4. Frontend: edit dialog and folder refresh wiring

- [x] 4.1 Create `src/components/FeedEditDialog.vue` (Teleport + overlay) with folder assignment dropdown, custom title input, read-only status (URL, last fetched, last error), Save/Cancel, and in-dialog error display; verify `npm run check` passes
- [x] 4.2 Wire the dialog in `App.vue`: the sidebar's Edit… event opens it for the selected feed; Save calls `moveFeedToFolder`/`setFeedTitle` only when changed, then reloads; verify `npm run check` passes
- [x] 4.3 Wire folder refresh: Sidebar menu emits a folder-refresh event, `App.vue` calls `refreshFolder`, shows a summary alert "X feed(s) refreshed, Y failed", and reloads; verify `npm run check` passes

## 5. Verification

- [x] 5.1 Run `cargo test` in `src-tauri` and `npm run check`; verify all Rust tests pass including the new custom-title and folder-refresh tests, and vue-tsc reports no errors
- [x] 5.2 Verify observable behavior in the running app (`npm run tauri dev`): rows show only name/badge/⋮, both menus offer exactly the specified items, the edit dialog shows folder/title/status and persists a custom title across a refresh, and a folder refresh shows the summary alert; verify `openspec validate row-menus-folder-refresh-feed-edit --strict` reports no errors
