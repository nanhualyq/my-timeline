## Context

Sidebar rows currently render all actions inline: a feed row has a refresh button, a move-to-folder `<select>`, and a remove button inside `.feed-actions` (a `flex-shrink: 0` span), which steals width from the ellipsized `.feed-title`; a folder row has rename (✎) and delete (×) buttons. Refresh architecture: `reader::refresh_all` collects target `(id, url)` pairs and drives them through a `FuturesUnordered` capped at `MAX_CONCURRENT_FETCHES = 5`, returning `(feed_id, Result)` per feed; `refresh_feed` handles one feed. `feeds.title` is overwritten on every fetch by `insert_or_update_feed`'s `ON CONFLICT(url) DO UPDATE SET title = ...`. The `COLUMN_MIGRATIONS` array in `db.rs` already provides idempotent, re-runnable `ALTER TABLE ... ADD COLUMN` statements. `FeedDto.title` feeds the sidebar, the reading pane, the remove confirmation, and OPML export. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**

- Folder and feed rows show only name + unread badge + a ⋮ menu control; all row actions live in the menu.
- Folder refresh with a single summary alert ("X refreshed, Y failed").
- Feed edit dialog: folder assignment, persistent custom title, read-only status.

**Non-Goals:**

- Moving app-level controls (refresh all, OPML import/export, entry-list actions) into menus.
- Automated UI/E2E test infrastructure (verified via `vue-tsc`, Rust tests, and manual dev-server checks, as in previous changes).
- Per-entry or reading-pane editing.
- Fixing the pre-existing edge where an OPML-imported custom title becomes the stored site title (see Risks).

## Decisions

1. **Custom title as a nullable `custom_title` column; display title = `COALESCE(custom_title, title)`.** `title` stays the auto-managed feed-provided value, so the fetch path (`insert_or_update_feed`) needs no changes and reverting a custom title is just `SET custom_title = NULL`. Alternative considered: a `title_custom` boolean flag guarding the `title` column — rejected because clearing the custom title would have no original title left to restore. `list_feeds` and `get_feed` select `COALESCE(f.custom_title, f.title) AS title`, and sidebar ordering moves to `ORDER BY COALESCE(f.custom_title, f.title) COLLATE NOCASE` so the list follows displayed names; OPML export reads the same display title. New `db::set_feed_title(conn, feed_id, Option<&str>)` sets or clears it.
2. **Folder refresh is a backend command, `refresh_folder(folder_id)`.** Refactor `refresh_all`'s bounded-concurrency loop into a helper that takes a target list; `refresh_all` passes all feeds, `refresh_folder` passes the folder's feeds (ids + urls from one query, erroring for an unknown folder). Returns the same `Vec<RefreshOutcome>` shape as `refresh_all`, so the frontend counts successes/failures and shows the summary alert. Alternative: the frontend looping `refreshFeed` invokes — rejected: sequential fetches, N IPC round-trips, and no aggregate result to summarize.
3. **Hand-rolled dropdown menu component, no new dependency.** A small `RowMenu.vue` (a ⋮ trigger button plus an absolutely positioned panel) that closes on outside click, Escape, and after an item is chosen. Rows pass their items; the panel is right-aligned under the trigger and layered above sibling rows. A library (e.g., floating-vue) was rejected as overkill for one menu pattern.
4. **Feed edit dialog as a modal component (`FeedEditDialog.vue`, Teleport + overlay).** Props: the feed and the folder list; local draft state for folder id and custom title; Save issues `move_feed_to_folder` only when the folder changed and `set_feed_title` only when the title changed, then emits `changed` so the app reloads. Status fields render read-only from `FeedDto`. Dialog errors are shown inside the dialog. Remove stays in the row menu per the spec.

## Risks / Trade-offs

- [COALESCE must be applied in every query that returns a display title; missing one shows the stale site title.] → Only `list_feeds` and `get_feed` return `FeedDto`; tests cover both, and a grep for `f.title` guards regressions.
- [Menu/dialog accessibility: focus handling, Escape, outside-click.] → Use real `<button>` triggers, close on Escape/outside click, and keep confirmations (`confirm()` for remove/delete) unchanged.
- [An OPML import stores the exported custom title as the site `title`, so a later refresh replaces it and the display reverts.] → Accepted known limitation (Non-Goal); fixing it would require exporting/importing a separate custom-title attribute.
- [`refresh_folder` duplicates fetch orchestration if left unshared.] → Factor the `FuturesUnordered` loop into one helper used by both commands.

## Migration Plan

The `custom_title` column is added through the existing `COLUMN_MIGRATIONS` mechanism (idempotent, tolerates re-runs) on the next app start. Rollback is a plain git revert; the unused column is harmless to older code.

## Open Questions

None.
