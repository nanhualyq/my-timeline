## 1. Schema and migration

- [x] 1.1 Add `app_meta(key TEXT PRIMARY KEY, value TEXT)`, `sync_marks`, and `sync_tombstones` to the schema in `src-tauri/src/db.rs`, wired into `init_schema` so they are created on every open; verify by a `db.rs` test that opens an in-memory database and asserts all three tables exist.
- [x] 1.2 Add `folders.uuid TEXT` through the existing `COLUMN_MIGRATIONS` mechanism, backfill random hex for existing rows, and add a unique index; verify by a test that opens a database created before the migration (folder row inserted, then `uuid` dropped) and asserts every folder ends up with a distinct non-empty uuid.
- [x] 1.3 Add `feeds.sync_ctr INTEGER NOT NULL DEFAULT 0` and the matching counter column for folders through the same mechanism; verify by a test asserting the columns exist and default to 0 on pre-existing rows.
- [x] 1.4 Write `device_id` into `app_meta` once per database and expose `next_counter()`; verify by a test asserting the id is stable across two opens and that `next_counter()` is strictly increasing.

## 2. Identity and local edit stamping

- [x] 2.1 Make folder creation assign a uuid and `sync_ctr`, and make feed creation assign `sync_ctr`; verify by tests asserting two folders created with the same name get different uuids and increasing counters.
- [x] 2.2 Add per-field counters to feed custom title and folder assignment, stamped from `next_counter()` on each local change; verify by a test asserting `set_feed_title` and `move_feed_to_folder` each bump only their own field's counter.
- [x] 2.3 Confirm the existing read path is untouched: run the full existing `db.rs` test module and the Vitest suite and confirm no query changes were needed for `list_feeds`, `list_entries`, `count_entries`, `entry_counts`, `mark_scope_read`.

## 3. Marks ledger

- [x] 3.1 Add ledger read/write in `src-tauri/src/db.rs` (write a mark row, read all marks, prune by day) and derive `day` from `COALESCE(published_at, created_at)`; verify by a test asserting an entry's day matches its sort date and that pruning drops only rows before the given day.
- [x] 3.2 Implement one `apply_marks_to_entries` function that materializes ledger rows onto `entries.is_read` / `entries.is_starred` and skips entries not present locally; verify by a test that records a mark for an absent entry, fetches that entry, and asserts the mark is materialized without touching unrelated rows.
- [x] 3.3 Write the ledger row and the entry column in the same transaction inside `set_read` / `set_starred`, and record nothing for entries older than the read horizon; verify by tests asserting (a) a recent entry produces both writes, (b) an entry older than the horizon produces only the entry write.
- [x] 3.4 One-time seed of the ledger from existing entries inside the horizon plus all starred entries, guarded by an `app_meta` flag, with counter 0; verify by a test that seeds from a pre-existing library, asserts stars and recent read marks are present with counter 0, and asserts a second open seeds nothing further.

## 4. Removal propagation

- [x] 4.1 Record a tombstone when a feed or folder is removed, keeping the delete itself hard, and prune tombstones older than 90 days; verify by tests asserting `delete_feed` leaves a tombstone row, cascades its entries as before, and that a tombstone older than the retention is pruned.
- [x] 4.2 Implement the merge rule `row survives iff row.sync_ctr > tombstone.ctr` as a pure check in `src-tauri/src/sync.rs`; verify by unit tests covering: tombstone beats a stale listing, re-add beats an older tombstone, and no-tombstone always survives.
- [x] 4.3 Confirm local add-feed and create-folder are unaffected by tombstones and produce a counter that outranks them; verify by a test that deletes a feed, adds the same URL again, and asserts it is listed and survives a merge with a document that recorded the deletion.

## 5. Document format and merge

- [x] 5.1 Define the serde structs and serialization for the document (`doc_version`, `read_horizon_day`, `folders`, `feeds`, `marks`, reserved `settings`) in `src-tauri/src/sync.rs`; verify by a round-trip test asserting serialize→parse is lossless and that entry content never appears in the serialized text.
- [x] 5.2 Implement `project(conn) -> Document`; verify by a test asserting feeds key by URL, folders by uuid, marks key by `(feed_url, guid)`, and that no local integer id appears in the serialized output.
- [x] 5.3 Implement pure `merge(a, b) -> Document` with per-field counter resolution and horizon `max`; verify by property tests over generated document pairs asserting order-independence (merge(a,b) == merge(b,a)) and idempotence (merge(d,d) == d).
- [x] 5.4 Implement `apply(doc, conn) -> ImportSummary` counting added/changed folders, feeds, stars, read marks, and marks recorded for later arrival; verify by a test asserting applying the same document twice reports changes on the first pass and zero on the second.
- [x] 5.5 Reject unsupported `doc_version` and unparsable text before any write; verify by tests asserting the library is byte-identical after a rejected import.

## 6. Commands

- [x] 6.1 Add `export_sync_document` and `import_sync_document` commands in `src-tauri/src/lib.rs` following the existing `shared_conn` + `Result<_, String>` pattern, registered in the existing `generate_handler!` list; verify with `cargo check` and by asserting the export command's returned text parses as a document.
- [x] 6.2 Add `cargo test` coverage for the command-level shape (export on an empty library, import of a document into an empty library) so the plumbing is exercised without the UI; verify by the tests passing. The command bodies are exercised through `export_sync_outcome` / `import_sync_outcome`: the tauri mock runtime cannot be used here because enabling `tauri/test` makes the test binary fail to start on Windows (`STATUS_ENTRYPOINT_NOT_FOUND`), so the seam sits just below the state lookup instead of the whole command.

## 7. Frontend

- [x] 7.1 Add the document types and `exportSyncDocument` / `importSyncDocument` wrappers to `src/api.ts`; verify with `npm run check`.
- [x] 7.2 Create `src/lib/sync-file.ts` modeled on `src/lib/opml-file.ts`, including the Android MIME-type filter (`application/json`, `text/plain`, `application/octet-stream`) and a `my-timeline-sync.json` default name; verify with unit tests for the filter selection on an Android user agent versus desktop.
- [x] 7.3 Add Export Sync Document and Import Sync Document to `src/components/SettingsDialog.vue` with a result summary line and a rejection message; verify with component tests covering success, rejection, and a cancelled picker leaving the message area unchanged.
- [x] 7.4 Confirm the dialog still closes on Escape and on overlay click and does not alter the current selection; verify by the existing `SettingsDialog` tests still passing.

## 8. End-to-end verification

- [x] 8.1 Export from one database, import into a second empty database, and export again; verify the two exported documents are semantically identical and the second import reports no changes.
- [x] 8.2 Simulate two-device use with `open_memory`: star and read on A, rename a folder on B, merge both, apply to both, and verify each side ends up with the union and the correct per-field winners.
- [x] 8.3 Verify a mark imported for an entry that is not yet fetched survives a `refresh_feed` and lands as starred or read; verify by test.
- [x] 8.4 Run `npm test`, `npm run check`, and `cargo test` and confirm all pass.
