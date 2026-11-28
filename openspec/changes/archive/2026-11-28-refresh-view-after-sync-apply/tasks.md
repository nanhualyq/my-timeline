## 1. Backend: report whether the library changed

- [x] 1.1 Add `applied: bool` (`#[serde(default)]`) to `SyncStatus` in `src-tauri/src/sync.rs`, keeping `SyncStatus::now` as the "nothing applied" constructor and adding an `applied` constructor for the post-merge paths; verify `cargo test sync` still compiles and the existing status tests pass unchanged
- [x] 1.2 Thread the accumulated `!nothing_applied(&summary)` result through every `SyncStatus` created after a merge point in `cycle` (the `unchanged` branch, the written branch, the created branch, and the `give up after SYNC_ATTEMPTS` branch), OR-ing it across write-conflict retries; verify with a unit test that a `PreconditionFailed` retry still reports `applied = true` when the first attempt imported records
- [x] 1.3 Leave the pre-merge failure paths (`get_document` error, unreadable document, database lock) reporting `applied = false`; verify with a test that a transport failure reports `applied = false` and keeps its `Failed` outcome
- [x] 1.4 Add a cycle test where the remote document carries a subscription and a star the local library lacks and nothing needs uploading, asserting `outcome == NothingChanged`, `applied == true`, one conditional read, and no `PUT`; run `cargo test` in `src-tauri`
- [x] 1.5 Add a test that a local-only upload reports `outcome` `synced`/`created` with `applied = false`, and that a replayed status (focus throttle and already-running paths) preserves the recorded `applied` value; verify with `cargo test`
- [x] 1.6 Add a serde round-trip test showing a status JSON written without the `applied` key deserializes with `applied = false`; verify with `cargo test`

## 2. Frontend: refresh on the new signal

- [x] 2.1 Add `applied: boolean` to the `SyncStatus` interface in `src/api.ts` and update the fake backend's default status in `src/__tests__/fakeBackend.ts`; verify `npm test` still passes
- [x] 2.2 Change `appliedChanges` in `src/lib/background-sync.ts` to `status.applied === true` OR the legacy `synced`/`created` outcomes, and update `src/__tests__/backgroundSync.spec.ts` so a `nothing_changed` status with `applied: true` refreshes while `applied: false`, `failed`, and `null` do not; verify with `npm test`
- [x] 2.3 Replace the inline outcome check in `SettingsDialog.runSync` with a call to `appliedChanges`, so "Sync now" emits `changed` when the sync applied changes but uploaded nothing; verify with a `syncSettings.spec.ts` case asserting the `changed` emit for `{ outcome: "nothing_changed", applied: true }` and no emit for `applied: false`
- [x] 2.4 Verify the app-level wiring refreshes once per applied sync and leaves the selection, filter, and loaded window untouched: extend `src/__tests__/app.spec.ts` (or `backgroundSync.spec.ts`) with a startup/focus path whose backend reports `applied: true`, asserting feeds, folders, counts, and rows are re-read; run `npm test`

## 3. End-to-end verification

- [x] 3.1 Run `npm test` and `cargo test` in `src-tauri`, and confirm both suites pass
- [x] 3.2 Manually verify in a dev run: seed the sync connection against a temporary server that already holds another installation's document, launch the app, and confirm the sidebar shows the downloaded folder and subscription without a restart (screenshots and request log kept under `%TEMP%\mt-verify`); record the result below

## Verification record

**Run:** 2026-11-28, `npx tauri dev` on Windows, one application window.

**Setup:** the dev database (`%APPDATA%\com.mytimeline.app-dev\reader.db`) was backed up, replaced with an empty one, and given a saved WebDAV connection pointing at a temporary local server (`127.0.0.1:8799`) that already held another installation's document: one folder (`Synced folder`) and one subscription (`https://synced.example/feed`) filed under it, with `read_horizon_day` equal to what this installation computes. The local library held no feeds and no folders, so the document needed no upload — the exact download-only case that used to be reported as "nothing happened".

**What the sync did:** one conditional `GET`, no `PUT`. The recorded status was

```json
{"outcome":"nothing_changed","detail":"1 feed(s) added, 1 folder(s) added","applied":true}
```

**What the reader saw:** before the sync the sidebar was empty (`before.png`); after it the sidebar listed `FOLDERS → Synced folder` with no restart, no dialog, and no manual refresh (`after.png`). The entry list stayed empty, as designed: entry bodies are re-acquired through the existing refresh actions.

**Artifacts:** screenshots, the request log, and the throwaway server were kept under `%TEMP%\mt-verify` for the duration of the check and then deleted, along with the empty database and the backup. The dev database was restored from its backup before that (38 feeds, no sync connection, unchanged size), and the running release build the reader already had open was left alone.
