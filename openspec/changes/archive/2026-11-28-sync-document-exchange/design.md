## Context

See `proposal.md` — Why.

The current state that shapes this design:

- Each installation keeps its own SQLite database; there is no shared state of any kind today.
- Primary keys are local `INTEGER PRIMARY KEY` values. The only copyable identities are `feeds.url` (UNIQUE) and `entries(feed_id, guid)` (UNIQUE). `folders` has only a `name`, with no uniqueness.
- `entries` holds both content (re-fetched per device) and intent (`is_read`, `is_starred`), so intent currently cannot exist independently of a fetched entry.
- `delete_feed` and `delete_folder` are hard deletes; `feeds → entries` cascades. Nothing records that a removal happened.
- There is no dirty tracking, no `updated_at`, and no settings persistence at all.
- The file-picker path already exists for OPML (`src/lib/opml-file.ts` + `tauri-plugin-dialog` + `tauri-plugin-fs`), including Android's MIME-type quirk. The Rust side never touches the filesystem for interchange; it only ever sees text.

## Goals / Non-Goals

**Goals:**

- A document format and merge rules that are provably deterministic, order-independent, and idempotent, so that the same rules can later be reused unchanged over a network transport.
- Intent stored independently of entry arrival, keyed by a copyable identity.
- Removal that propagates.
- Migration of existing databases with no data loss and no change to any existing query.
- The picker, dialog, and command shape mirror the existing OPML path closely enough that the reader recognizes the flow.

**Non-Goals:**

- WebDAV transport, endpoint configuration, credentials storage, conditional PUT, retry loops.
- Any automatic triggering: no sync on start, on focus, on a timer, or after a write.
- Client-side encryption. The document is plaintext; confidentiality is the storage's responsibility. This is a deliberate acceptance: the local database already stores the full reading history in plaintext, so encrypting only the exported file protects little.
- Settings persistence. The document format reserves a `settings` section, but this change neither writes nor reads it.
- Conflict UI. Resolution is silent.
- Sharing entry content, excerpts, cover images, fetch state, or feed-provided titles (a feed's title is re-derived from fetch; only the reader's custom title is shared).

## Decisions

### D1: Intent lives in its own table; `entries.is_read` / `entries.is_starred` stay as a materialized read path

```
   sync_marks (feed_url, guid, day, read, read_ctr, star, star_ctr)
        |                                   ^
        | apply_marks()  (one function)     | local set_read / set_starred
        v                                   | writes both
   entries.is_read / entries.is_starred ----+

   所有读路径 (list_entries, entry_counts, mark_scope_read, ...) 仍然只读 entries
```

`sync_marks` keys an intent by `(feed_url, guid)`, so a star can be recorded before the entry exists locally, and is applied when it arrives.

Alternatives considered:

- *Add counters to `entries` and keep it the only source of truth.* Rejected: an imported mark for an entry the device has not fetched has no row to attach to, so the mark would be dropped, and the second device would silently narrow the shared state.
- *Fully normalize (all reads join the ledger).* Rejected: it rewrites `list_entries`, `count_entries`, `entry_counts`, `mark_scope_read`, and the keyset index, which is the app's hot path, to gain nothing.

The mitigation for the obvious risk (two places to write) is structural: exactly one function applies the ledger to `entries`, and both the import path and the post-fetch reconcile path call it. Local `set_read` / `set_starred` write the entry columns and the ledger row in the same transaction.

### D2: Ordering is a monotonic counter, never a wall clock

`app_meta` holds `device_id` and `counter`. Every local edit that is shareable does `counter += 1` and stamps the field with the new value. An imported document first raises the local `counter` to above every value it contains, so an edit made after seeing a remote value always outranks it.

Using `(timestamp, device_id)` pairs instead was rejected: multi-device clock skew is normal, and a phone whose clock is two hours fast would win every field forever. `Last-Modified`/`ETag` from a future transport are for "did the bytes change", not for "which value is newer".

Each field carries its own counter, not one counter per record, so that renaming a feed on one device and moving it to another folder on the other keeps both changes.

Absent and `false` are distinct for `read` and `star`: the document omits a field that was never set. Without that distinction, "mark unread on the phone" cannot be represented and would be reinstated by any device that has read the entry.

### D3: Removal writes a tombstone; deletes stay hard deletes

`sync_tombstones(kind, key, ctr)` records the removal. `feeds` and `folders` gain `sync_ctr`, the counter at which the current row was created.

```
   feed survives a merge  <=>  feed.sync_ctr > tombstone.ctr   (or no tombstone)
```

Soft-deleting (`feeds.deleted_at`) was rejected: every existing query — `list_feeds`, unread counts, the folder join, the entry keyset queries — would need a `WHERE deleted IS NULL` guard, and a missed guard shows deleted feeds.

Hard delete plus tombstone keeps every query untouched. Local re-add does not consult or clear the tombstone: it inserts with `sync_ctr = counter + 1`, which already outranks the tombstone everywhere, so "delete on A, re-add on B" converges to present on both.

Folders are asymmetric: a tombstone for a folder always wins over a remote rename of it. Renaming a folder that another device deleted is a case with no good answer, and dropping it is the cheaper mistake than resurrecting a folder the reader deleted.

### D4: Folders get a UUID; same-named folders are not merged

`folders` gains `uuid TEXT` (random hex, backfilled, unique). Folders are matched across devices by UUID, never by name, because the name is mutable and not unique.

Consequence: two devices that each create a folder named "Tech" while offline produce two folders named "Tech" after the merge. Merging by name was rejected — it is lossy (the two folders can hold different feeds intentionally) and it makes rename ambiguous. Showing a duplicate is visible and fixable by the reader; a silent merge is neither.

### D5: Read marks are bounded by a day recorded in the document

The document carries `read_horizon_day` (a calendar date). Marks whose `day` — the entry's sort day, `COALESCE(published_at, created_at)` — falls before the horizon are dropped.

The horizon is a field, not a computation, because `today − 7d` computed independently on two devices yields two different sets, and every merge would then look like a change. A writer advances it to `max(observed, own_today − 7d)`, capped at one day of advance per write, so a device with a badly skewed clock cannot jump the horizon forward and discard marks other devices still consider current.

Marks for entries older than the horizon are never recorded in the first place — filtering at write time, not only at export, keeps the ledger from accumulating rows that are discarded hours later.

Stars are exempt from the horizon. The document therefore grows with the reader's starred set, which is the intended permanent part.

Intent is stored as a flat array with a `day` field rather than nested day buckets. Bucketing was the earlier sketch; it buys O(1) whole-bucket discard but complicates the merge and the per-record counters, and at a 7-day horizon the array is small enough that a filter costs nothing.

### D6: The document is one plain JSON object with an explicit version

`doc_version` is the first field. A document with a version this build does not support is rejected with a message naming it; nothing is partially applied. Format version is separate from the app version so that a format change is a deliberate decision.

`settings` is reserved and absent in this change. Adding it later does not need a version bump — an absent section means "not present", which is exactly what an older document says.

Import is merge-only: it never removes local data, never fetches a feed, and reports counts. It reuses the renderer/parser split that OPML uses (Rust produces and consumes text; the frontend picks the file), so `src/lib/sync-file.ts` is a near-copy of `opml-file.ts`, including the Android MIME-type workaround (`application/json`, `text/plain`, `application/octet-stream` alongside the `json` extension).

### D7: `sync.rs` holds the merge as pure functions

Structs plus `merge(a, b) -> doc`, `project(db) -> doc`, `apply(doc, db)`. No I/O and no `reqwest`; the commands in `lib.rs` do the plumbing in the same style as the existing ones (`shared_conn`, `Result<_, String>`).

Keeping merge free of the database is what makes the order-independence and idempotence claims testable as property tests over arbitrary document pairs — those properties are the whole reason a network transport can later be added without re-designing anything.

## Risks / Trade-offs

- **Dual write between the ledger and the entry columns** → one apply function, called from import and from post-fetch reconcile; a test asserts that after any sequence of local edits plus fetches the two agree for every entry inside the horizon.
- **Skewed clock advances the horizon** → the horizon only moves forward, by at most one day per write, and only drops marks older than 7 days. Worst case a device with a fast clock causes up to a few extra days of marks to be dropped, never a corrupt merge.
- **Duplicate same-named folders after an offline merge** → accepted and visible; the reader can delete one. Avoiding it would require lossy name matching.
- **Folder deletion beats remote rename** → accepted asymmetry, documented above.
- **Tombstones accumulate** → pruned after 90 days by the recorded day. This must exceed the longest realistic period a device stays offline; a device offline longer than that can resurrect feeds it deleted. Chosen over unbounded growth.
- **The document reveals the reading-interest profile in plaintext** → accepted; the same data is already plaintext in the local database. Encryption would turn the document into an opaque blob and force whole-file replacement, which is a different design.
- **A mark recorded for an entry that never arrives keeps a ledger row until the horizon passes it** → bounded by the horizon; harmless.

## Migration Plan

Applied on database open, alongside the existing idempotent `COLUMN_MIGRATIONS`:

1. `ALTER TABLE folders ADD COLUMN uuid TEXT`; backfill random hex; create a unique index.
2. `ALTER TABLE feeds ADD COLUMN sync_ctr INTEGER NOT NULL DEFAULT 0` (and the same on the tombstone-bearing table for folders).
3. Create `app_meta`, `sync_marks`, `sync_tombstones`.
4. Write `device_id` once if absent.
5. One-time seed of `sync_marks` from existing `entries` inside the horizon plus all starred entries, guarded by an `app_meta` flag so it runs once. Seeded rows get counter `0` — the weakest possible — so any real edit or import outranks them. Without the seed, the ledger and the entry columns would disagree on day one and the first import would appear to lose the reader's existing stars.

Rollback is safe in both directions: an older build ignores the new tables and columns, and hard deletes remain hard deletes. No existing row is modified except `folders.uuid` and the seed, both additive.

## Open Questions

- Whether the document should carry a revision counter now (useful for a future transport to skip work cheaply) or whether that belongs to a `doc_version` 2 when a transport exists.
- Whether tombstone retention should be reader-configurable. Deferred; 90 days is a constant for now.
