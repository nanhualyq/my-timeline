## Why

The sync document only travels when the reader manually carries a file between machines, so in practice the second installation is almost always stale: a star made on the phone is invisible on the desktop until someone remembers to export and import. The document format, the merge rules, and the ordering that resolves conflicts already exist and are tested; what is missing is a wire and a moment to use it.

A WebDAV collection is the smallest possible transport for this: it is a plain file store the reader already runs (Nextcloud, Synology, a self-hosted server), it needs no service to operate, and it holds exactly one document.

## What Changes

- Add **WebDAV connection settings**: server URL, remote path, username, password, and an opt-in "accept invalid certificates" toggle for self-signed servers. Settings and password are stored in the local database, consistent with what it already holds.
- Add a **sync cycle**: fetch the remote document, merge it into the local library, then upload the merged result. The upload is conditional on the version that was read, and a lost race is re-read and re-merged rather than overwriting the other installation's changes.
- Add **change detection**: a conditional GET that resolves nothing to download when the remote document is unchanged, and a canonical comparison of the merged document against what was last uploaded so an unchanged library produces no upload at all.
- Add **triggers**: once at startup, when the window regains focus (throttled to five minutes), and from a "Sync now" button. No background timer.
- Add **sync status** to the Settings dialog: when the last sync ran, what it did, and the reason it failed.
- Add a **connection test** so a wrong URL, path, or password is reported at configuration time rather than at the next sync.
- Make the document's **serialization canonical** so that two installations holding the same state produce identical text — the property the "is anything different" comparison rests on.

## Capabilities

### New Capabilities

- `webdav-sync`: keeping the sync document on a WebDAV collection — connection settings and how they are stored, the conditional read/merge/conditional write cycle, what counts as a change, when a sync runs, and what the reader is told about it.

### Modified Capabilities

- `sync-document`: adds the requirement that rendering the same shared state produces identical text, which is what lets a transport decide that there is nothing to send.
- `settings`: the Settings dialog gains the WebDAV connection form, the sync status line, and the "Sync now" action, alongside the existing subscription-management and document import/export actions.

## Impact

- `src-tauri/src/webdav.rs` (new): the WebDAV client — GET/HEAD/PUT with conditional headers, PROPFIND-free existence check, error rendering. Reuses the `reqwest` client configuration the feed fetcher already uses, plus the invalid-certificate toggle.
- `src-tauri/src/sync.rs`: a canonical rendering used by both export and the "did anything change" comparison.
- `src-tauri/src/db.rs`: a `sync_config` table (URL, path, username, password, certificate toggle) and the last-sync record (time, outcome, the version last uploaded).
- `src-tauri/src/lib.rs`: commands for reading and writing the configuration, testing the connection, running a sync, and reading the last-sync status.
- `src/api.ts`, `src/components/SettingsDialog.vue`: the connection form, status line, and "Sync now" button; a startup and focus trigger in `src/App.vue`.
- `openspec/specs/webdav-sync/spec.md` (new), plus deltas to `sync-document` and `settings`.
- No new dependencies: `reqwest` is already a dependency with `rustls-tls` and `system-proxy`.
- Out of scope: client-side encryption of the document, background timers, per-device document shards, conflict prompts, WebDAV Digest authentication, and any server component.
