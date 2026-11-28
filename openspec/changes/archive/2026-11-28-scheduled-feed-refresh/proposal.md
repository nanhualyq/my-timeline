## Why

Fetching feeds is a fully manual act today: the reader has to press a button, there is no way to keep the library fresh on its own, and a long refresh gives no feedback beyond a button label changing — the folder refresh even blocks the app behind a native modal that reports only "N refreshed, M failed". Once a library holds a few dozen feeds, a manual full pull is both slow and opaque, and on a phone the text button in the list header is the wrong control entirely.

## What Changes

- Add a scheduled refresh: an on/off switch plus an interval, persisted locally, that fetches every feed whose content is older than the interval. It runs while the app is visible and catches up when the reader returns.
- Replace the fire-and-forget refresh commands with a single frontend-driven refresh runner shared by every trigger (the list action, the `r` keystroke, the new schedule, returning to the foreground, and the new pull gesture). The runner is single-flight, reports per-feed progress as it goes, and can be stopped.
- Add a floating refresh panel: live progress, a per-feed result list, failures with their reasons, a retry for the failed feeds, and a completion summary that includes how many new entries arrived.
- Fetch politely per host: the runner never has more than one request in flight to the same hostname, spreads the first requests across different hosts, and backs off a host that answers with a rate-limit or server-unavailable status instead of reporting every feed on it as failed.
- **BREAKING**: `refresh_all` and `refresh_folder` are removed from the Rust command surface. The concurrency policy now lives in the frontend, in one implementation instead of two.
- Remove the "Refresh all" text button from the entry list on the one-pane layout, where a downward pull at the top of the list refreshes the current list's scope instead.
- Remove the modal dialog that reported a folder refresh; the panel reports outcomes for every scope.
- Settings gains a Refresh section: the switch, the interval, the last automatic refresh, and when the next one is due.
- Refreshing a feed reports how many entries it newly inserted, and the whole feed store now runs in one transaction instead of one implicit transaction per entry.

## Capabilities

### New Capabilities

- `feed-refresh`: the scheduled refresh (switch, interval, due-eligibility, visibility behavior, local persistence), the single refresh runner shared by every trigger, its progress reporting, politeness toward a single host, stop/retry, and the refresh results surfaced to the reader.

### Modified Capabilities

- `rss-reader`: the manual refresh requirements stop being two separate all-feeds/one-feed actions driven by backend commands; they become scoped runs of the shared runner whose outcomes appear in the refresh panel, and the one-pane layout no longer offers a text refresh button.
- `settings`: the Settings dialog gains a Refresh section (switch, interval, last automatic refresh, next due time).
- `touch-gestures`: a downward pull at the top of the entry list refreshes the current list's scope, and its mechanics must not collide with the horizontal row drags, with vertical scrolling, or with the long-press row menu.

## Impact

- `src-tauri/src/reader.rs`: `refresh_all`, `refresh_folder`, `refresh_targets`, `MAX_CONCURRENT_FETCHES`, and `RefreshOutcome` are deleted; a per-feed refresh returns a richer result (error, new-entry count, HTTP status).
- `src-tauri/src/lib.rs`: `refresh_all` and `refresh_folder` commands removed; `refresh_feed` returns the richer result; new commands to load and save reader settings.
- `src-tauri/src/fetch.rs`: `fetch_and_store` reports the inserted count and the response status; `store_feed` wraps the feed header, its entries, and the mark application in one transaction.
- `src-tauri/src/db.rs`: a `(id, url)`-based local settings read/write over `app_meta`, plus the entry-count helper used to derive the inserted count. No schema migration and nothing added to the sync document.
- `src/lib/refresh.ts` (new): the runner — due-eligibility, host-aware scheduling, single-flight, cancellation, and progress events.
- `src/components/RefreshPanel.vue` (new) and a shared notice stack that also hosts `UndoBar`.
- `src/components/EntryList.vue`: the text refresh action leaves the one-pane layout and the pull gesture replaces it.
- `src/lib/gestures.ts`: a vertical pull recognizer alongside the existing horizontal drag machine.
- `src/components/SettingsDialog.vue`: the Refresh section.
- `src/App.vue`: every trigger path funnels into the runner; the foreground-return refresh joins the same due-eligibility logic; the folder-refresh modal is gone.
- `src/api.ts`: `refreshAll`/`refreshFolder` removed, `refreshFeed` returns the richer result, reader-settings load/save added.
- Tests: `src/__tests__/backgroundSync.spec.ts` and `resumeRefresh.spec.ts` neighbors gain runner coverage; the Rust tests covering the deleted commands are removed.
