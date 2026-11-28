## 1. Rust: the per-feed refresh primitive

- [x] 1.1 Add `FeedRefresh { feed_id, error, new_entries, http_status }` to `src-tauri/src/reader.rs` and make the `refresh_feed` command return it, keeping the current success/failure recording on the feed row; verify `cargo test` in `src-tauri` still passes and the command's serde shape matches the TypeScript interface added in 6.1
- [x] 1.2 Extend the fetch path so a failed fetch carries its HTTP status code to the caller instead of only a formatted string, and so a successful fetch reports success with no status; add a Rust unit test covering a non-success status reaching the caller as a status code
- [x] 1.3 Count newly inserted entries — wrap the feed upsert, its entries, and the mark application in one transaction and derive the count from `SELECT COUNT(*) FROM entries WHERE feed_id = ?` taken inside that transaction before and after the store; verify with a Rust test that a first fetch reports the entry count and an unchanged re-fetch reports zero, and that the transaction leaves no partial feed on a storage error
- [x] 1.4 Delete `refresh_all`, `refresh_folder`, `refresh_targets`, `MAX_CONCURRENT_FETCHES`, and `RefreshOutcome` from `reader.rs`/`lib.rs` along with the command registrations and their Rust tests; verify `cargo test` passes and `grep` finds no reference to the removed names

## 2. Rust: locally stored refresh settings

- [x] 2.1 Add `load_refresh_settings` / `save_refresh_settings` commands over the existing `app_meta` key/value accessors, storing one `refresh_settings` JSON row (`enabled`, `interval_minutes`), with a missing row reading as disabled with a 30-minute interval; verify with a Rust test covering read-after-write, the default when absent, and rejection of an interval outside the offered set
- [x] 2.2 Verify the setting is device-local by asserting the sync document's export path does not read `refresh_settings`; add or extend a `sync.rs` test that exports a library with a saved setting and finds no trace of it in the document

## 3. Frontend: the refresh runner

- [x] 3.1 Add `src/lib/refresh.ts` with the run model: targets for a scope, due-eligibility from `last_fetched_at` plus the per-feed jitter, the dispatched-feed set that makes a run single-flight, `cancel()`, and a progress callback; verify with Vitest cases for an empty due set, a partly due set, and a second trigger adding only feeds the run does not cover
- [x] 3.2 Implement host-aware dispatch inside the runner: group targets by hostname, round-robin the groups into the queue, keep one request in flight per host plus the inter-request gap, and bound total parallelism (5, or 3 on a touch layout); verify with Vitest cases asserting no two in-flight requests share a host, that a host-heavy queue still starts other hosts' feeds immediately, and that the in-flight count respects the limit
- [x] 3.3 Implement the 429/503 path: mark the host's remaining feeds deferred, hold them behind a delay that grows per refusal (60 s, then 300 s), continue other hosts, and retry the deferred feeds if the run is still live; verify with Vitest cases for deferral instead of per-feed failure, a successful retry clearing the error, and the second delay exceeding the first
- [x] 3.4 Return the run summary (`total`, `ok`, `failed`, `newEntries`, `stopped`) and per-feed results from the runner, and make `cancel()` stop dispatch while keeping completed results; verify with Vitest cases for the summary of a clean run, a run with failures, and a stopped run
- [x] 3.5 Update `src/api.ts` — say `refreshFeed` returns the new result type, remove `refreshAll`/`refreshFolder` and `RefreshOutcome`, add `RefreshSettings` load/save — and extend `src/__tests__/fakeBackend.ts` so it can script per-feed failures, statuses, and inserted counts; verify `npm run check` passes and the runner tests run against the fake backend

## 4. Frontend: the refresh panel

- [x] 4.1 Add `src/components/RefreshPanel.vue` rendering the running state (completed count, finished feeds with outcomes, failures with reasons, Stop) and the finished state (feeds/entries/failed summary, Retry failed, dismiss), shown only after the run has been in flight briefly; verify with Vue Test Utils cases for the delayed appearance, the running render, and both finished renders
- [x] 4.2 Add a `NoticeStack` region hosting both the panel and the existing `UndoBar` so they stack rather than overlap, keeping both below dialogs and above the panes; verify with a test that mounts both notices at once and asserts neither overlaps the other, plus the existing `undo.spec.ts` still passing
- [x] 4.3 Wire the panel into `App.vue`: every trigger path calls the runner, the `busy` flag becomes the run's running state, the folder-refresh `message()` modal is deleted, and the retry action runs only the failed feeds; verify with an `app.spec.ts` case that a folder refresh reports in the panel and shows no dialog

## 5. Frontend: the schedule

- [x] 5.1 Add the 60-second tick with due-eligibility, paused while `document.hidden` and run once on becoming visible, and fold `resumeRefresh` into the same due check while keeping the existing hidden-threshold rule; verify with Vitest cases for "hidden does not fetch", "visible after the interval fetches the newly eligible feeds", "visible before the interval fetches nothing", and the unchanged short-hidden case
- [x] 5.2 Add the Refresh section to `src/components/SettingsDialog.vue` (switch, fixed interval select, last scheduled refresh, next due time), persisting on change through the new commands and reporting inside the dialog; verify with a `SettingsDialog` test that toggling and choosing an interval persist and come back on reopen

## 6. Frontend: the pull gesture and the one-pane layout

- [x] 6.1 Add a vertical-downward pull recognizer to `src/lib/gestures.ts` that requires the scroll port to be at its top, applies resistance, commits past a threshold, and springs back otherwise, as a machine separate from the existing horizontal one; verify with Vitest cases for commit, spring-back, and rejection when the port is scrolled
- [x] 6.2 Attach the pull to the entry list's scroll port, resolve its scope from the current selection (feed, folder, starred, all), and hand the run to the panel; verify with `touchGestures.spec.ts` cases for each scope and for a pull during a live run starting no second run
- [x] 6.3 Remove the header refresh button from the one-pane layout while keeping it on the wide layout, and confirm the sidebar's feed and folder menus still offer their refresh actions; verify with `narrowLayout.spec.ts` and `sidebar.spec.ts` assertions for both layouts

## 7. Verification

- [x] 7.1 Run `npm test`, `npm run check`, and `cargo test` in `src-tauri` and confirm all pass with no reference to the removed commands or types
- [x] 7.2 Walk the reader-visible behavior on the desktop build: a manual full refresh with one unreachable feed reports progress, a summary, and a working retry; `npm run build:app` succeeds
- [x] 7.3 Walk the same on the Android build: the one-pane layout shows no refresh button, a pull at the top refreshes the selected feed, the panel appears and stops cleanly; `npm run android:build` succeeds
- [x] 7.4 Run `openspec validate scheduled-feed-refresh --strict` and confirm the change is valid
