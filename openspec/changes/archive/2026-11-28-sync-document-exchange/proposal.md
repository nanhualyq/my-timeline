## Why

Multiple installations of the app already run side by side — desktop and Android each keep their own `reader.db` — so a subscription list, a star, or a read mark made on one device is invisible on the other, and the reader ends up maintaining the same library twice. The data that actually needs to be shared is small and user-authored (subscriptions, folder assignment, custom titles, stars, read marks, and the removal of a feed), while entry bodies are re-fetched per device and never need to travel.

This change establishes that shared shape and validates it end to end through a plain file the reader can carry between devices. A later change puts the same document on WebDAV; keeping the format separate from the transport means the merge rules can be proven with a file picker before any network client, credentials, or polling exists.

## What Changes

- Introduce a versioned **sync document**: a plain JSON file describing folders, feeds, per-entry stars and read marks by natural key, and tombstones for removed feeds and folders. Entry bodies, fetch timestamps, and error state are excluded.
- Add **field-level merge** of two documents: subscriptions and folder assignment union, stars and read marks resolve per field by a monotonic per-device counter, and a tombstone outranks a remote re-add. Merging is deterministic and idempotent.
- Add **identity that survives a copy**: `folders` gain a UUID, feeds stay keyed by URL, and entry marks stay keyed by `(feed URL, guid)`. Local integer ids never leave the device.
- Add a **mark ledger** so a star or read mark can be recorded for an entry the device has not fetched yet, and applied when the entry arrives.
- Add **write-side horizons**: read marks are kept for 7 days, stars are kept indefinitely, and tombstones outlive both. A mark for an entry older than the read horizon is never recorded.
- Change feed and folder removal from a bare delete to a delete that also **records a tombstone**, so a removal propagates instead of being undone by another device.
- Add **Export Sync Document** and **Import Sync Document** to the Settings dialog, using the existing file-picker path. Import is merge-only: it never removes local data, never fetches a feed, and reports what it applied.
- **No breaking changes.** Existing databases migrate on open; existing commands and views are unchanged.

## Capabilities

### New Capabilities

- `sync-document`: the portable shared-state document — what it contains, how two copies merge, how identity and horizons keep it small and unambiguous, and how the reader exports and imports one.

### Modified Capabilities

- `settings`: the Settings dialog gains Export Sync Document and Import Sync Document actions that report their outcome in the dialog, alongside the existing subscription-management actions.

## Impact

- `src-tauri/src/db.rs`: new tables and columns (device/meta, mark ledger, tombstones, `folders.uuid`, per-field counters), plus a tombstone-writing delete path and migrations for existing databases.
- `src-tauri/src/sync.rs` (new): document serialize/deserialize/merge/apply, kept free of I/O so merge rules are unit-testable.
- `src-tauri/src/lib.rs`: two new commands, `export_sync_document` and `import_sync_document`.
- `src/api.ts`, `src/lib/sync-file.ts` (new), `src/components/SettingsDialog.vue`: command wrappers, the picker half modeled on `src/lib/opml-file.ts` (including its Android MIME-type handling), and two dialog buttons with a result summary.
- `openspec/specs/settings/spec.md`: unchanged in this change's delta scope except for the new dialog actions.
- No new dependencies. No network code: WebDAV, configuration storage, and automatic triggering are deliberately out of scope and left to a follow-up change.
