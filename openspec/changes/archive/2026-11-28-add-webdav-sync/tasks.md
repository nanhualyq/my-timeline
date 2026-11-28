## 1. Configuration storage

- [x] 1.1 Add a single-row `sync_config` table (url, path, username, password, accept_invalid_certs) to the schema in `src-tauri/src/db.rs` with a `CHECK (id = 1)` constraint; verify by a test that a second insert is rejected and that the table exists on an older database after `init_schema`.
- [x] 1.2 Add read/write helpers for that row and for the per-path `app_meta` records (`last_etag`, `last_uploaded_text`, `last_sync`), keyed so a changed remote path does not inherit the old path's etag; verify by a test that writing a new path leaves the previous path's records unread.
- [x] 1.3 Resolve the remote document URL from the configured server URL and path, falling back to a default document name when the path is empty and rejecting a server URL that is not http(s); verify with tests covering the empty path, a path with and without a leading slash, and a non-http scheme.

## 2. Canonical rendering

- [x] 2.1 Order `sync::project`'s marks by `feed_url, guid` so it matches the order `merge` produces; verify by a test asserting the two renderings of the same state are byte-identical.
- [x] 2.2 Add a `render(&Document) -> String` used by both export and the change comparison, so there is one canonical form rather than two call sites that could drift; verify by a test that exporting twice with no change in between yields identical text.

## 3. WebDAV client

- [x] 3.1 Create `src-tauri/src/webdav.rs` with a client built per call from the configuration (timeout, user agent, basic auth, and `danger_accept_invalid_certs` when the switch is on), and a test that the switch is only applied to this client.
- [x] 3.2 Implement `get_document(if_none_match) -> NotModified | Fetched { text, etag }` and `put_document(text, if_match) -> Written { etag } | PreconditionFailed`; verify with an in-process HTTP server (the pattern `reader.rs` tests already use) asserting the request headers carry `If-None-Match` / `If-Match` and that 304 and 412 are returned as those outcomes rather than as errors.
- [x] 3.3 Map transport failures onto named reasons — unreachable host, rejected credentials, missing collection, certificate rejected, other HTTP status — reusing the error-flattening approach in `fetch.rs`; verify with tests that a 401 and a TLS failure produce distinct messages.
- [x] 3.4 Implement `test_connection` (a `GET` or `HEAD` of the configured document) reporting reachability, credential acceptance, and whether the document exists; verify with tests against a server answering 200, 404, and 401.

## 4. Sync cycle

- [x] 4.1 Implement the cycle in `src-tauri/src/sync.rs`: read, merge into the library, compare against the last uploaded text, upload conditionally, and on a precondition failure re-read, re-merge and retry up to three attempts; verify with tests driving a stub server that (a) a concurrent write is re-read and both sides survive, (b) an exhausted retry budget reports failure and leaves the local library intact.
- [x] 4.2 Add the "nothing to do" paths: a 304 ends the cycle without downloading, and a merged text equal to the last uploaded ends it without writing; verify with tests asserting the server saw no `PUT` in the second case and no `GET` body work in the first.
- [x] 4.3 Create the document when it is missing, reporting that it was created; verify with a test against a server that answers 404 for the first GET and accepts the following `PUT`.
- [x] 4.4 Persist a `SyncStatus { ran_at, outcome, detail }` on every cycle including failures, in `app_meta`; verify by a test that a failed cycle leaves a status the reader can read afterwards.
- [x] 4.5 Guard the cycle so only one runs at a time, dropping a trigger that arrives during a run, and throttle non-forced runs to one per five minutes from the last attempt; verify with tests that a concurrent trigger is dropped and that a forced run bypasses the throttle.

## 5. Commands

- [x] 5.1 Add commands for reading and saving the configuration, testing the connection, running a sync (forced or not), and reading the last status, in `src-tauri/src/lib.rs`, following the existing `shared_conn` / `Result<_, String>` pattern; verify with `cargo check`.
- [x] 5.2 Register the commands in the existing `generate_handler!` list; verify by a cargo test asserting the exported handler description contains them, or by an explicit check that the list compiles with each name present.

## 6. Frontend

- [x] 6.1 Add the configuration, status, and outcome types plus wrappers to `src/api.ts`; verify with `npm run check`.
- [x] 6.2 Add the WebDAV section to `src/components/SettingsDialog.vue`: fields, save, test connection, "Sync now", and a status line naming when the last sync ran and what it did or why it failed; verify with component tests covering save success, a rejected credential message, a no-op status, and a failed status.
- [x] 6.3 Trigger a sync at startup after the first reload and when the window regains focus (also via `visibilitychange` returning to visible, for mobile), through one helper that ignores a not-configured connection and refreshes the sidebar, counts, and list after a sync that changed anything; verify with component tests that a not-configured connection triggers no command and that an applied sync reloads the list.

## 7. Verification

- [x] 7.1 Run `cargo test`, `npm test`, and `npm run check` and confirm all pass.
- [x] 7.2 Round-trip two in-memory libraries through a stub WebDAV server: star on one, subscribe on the other, sync both, and verify each ends with both and the server holds the merged document.
- [x] 7.3 Verify by test that a sync against an unreachable server leaves the library byte-identical and records a `Failed` status.
