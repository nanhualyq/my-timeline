## Context

See `proposal.md` — Why. The pieces this builds on already exist and are tested:

- `src-tauri/src/sync.rs` renders, merges, and applies the document; `merge` is a pure function and its order-independence and idempotence are covered by tests. `project`/`export`/`import` are the database bridge.
- `src-tauri/src/db.rs` holds the ordering counters (`app_meta.counter`), the mark ledger, and tombstones. `read_horizon_day` is stored rather than derived so two installations agree on the read window.
- `reqwest` is already a dependency with `rustls-tls`, `system-proxy`, and `gzip`, and `fetch.rs` already builds a client with a timeout and a user agent. `fetch.rs` also has the error-flattening helper (`describe`) that turns a `reqwest::Error` into the sentence that says which failure actually happened.
- The Settings dialog and its file-picker paths already exist; `src/lib/sync-file.ts` and `src/lib/opml-file.ts` established the pattern that the Rust side only ever handles text.
- There is no settings persistence of any kind and no network configuration anywhere.

WebDAV's relevant property: it is a file store with no merge and no cross-file transaction. A `PUT` replaces the whole document. `If-Match` / `If-None-Match` on `PUT` are in RFC 4918 and supported by Nextcloud, ownCloud, Synology and Apache mod_dav; they are the only concurrency primitive available.

## Goals / Non-Goals

**Goals:**

- One document on a WebDAV collection, kept converged across installations without the reader carrying files.
- No lost remote change: the upload is conditional and a lost race is re-read and re-merged.
- No wasted work: an unchanged remote document is not downloaded, an unchanged library is not uploaded.
- Failures are visible and attributable at the moment they happen and afterwards.

**Non-Goals:**

- Background timers, push, or long polling.
- Per-device document shards or per-record files.
- Conflict prompts: resolution stays silent, as the archive change decided.
- Client-side encryption of the document.
- WebDAV Digest authentication, locking (`LOCK`/`UNLOCK`), or `PROPFIND` beyond what a plain `HEAD`/`GET` answers.
- Any server-side component, and any change to the document format itself.

## Decisions

### D1: One document, conditional PUT, merge-and-retry — no sharding

```
   GET  <url>/my-timeline-sync.json      If-None-Match: <last seen etag>
        |  304 ······························► no body to download
        |                                      (the local side is still judged below:
        |                                       a 304 says nothing about local changes)
        v  200 + etag + body
   merge(remote, project(local))            pure, order-independent
        |
        v
   equal to the text last uploaded? ──yes──► nothing to send, stop
        |
        no
        v
   PUT  <url>/my-timeline-sync.json      If-Match: <etag from the GET>
        |  2xx ──────────────────────────────► record new etag + status
        v  412 Precondition Failed
   re-GET, re-merge, retry (at most 3 attempts)
        |
        v still failing → report, keep local state
```

Alternatives considered:

- *Per-device shards* (each installation writes its own file, everyone merges all of them). It eliminates the write-write race entirely, but it needs a directory listing, compaction, and tombstone accounting per file. With conditional PUT plus re-merge, the loss window shrinks to the interval between one GET and one PUT, which for two devices editing different fields is almost never hit — and when it is, the retry converges instead of losing. Rejected as unnecessary machinery.
- *Unconditional PUT with `Last-Modified` comparison.* Rejected: `If-Match` gives exactly the same protection with server-side support and no clock comparison.
- *Whole-file last-writer-wins.* Rejected: a single stale device would erase another's 50 stars, and the per-field merge is already written.

### D2: Exactly one sync at a time, owned by the backend

The sync runs behind a backend guard (a `Mutex<bool>` or a tokio `Mutex` held for the cycle), so a focus event arriving during a startup sync is dropped rather than queued. The frontend's triggers are fire-and-forget: they call a command and read the resulting status; they never decide whether a sync may run.

The focus throttle (five minutes) is also backend-side, recorded from the last completed sync attempt. Putting it in the frontend would make it reset on every reload, which is exactly when it matters least and misbehaves most.

### D3: The sync cycle is one command, and the frontend only reads status

`sync_now()` runs the whole cycle and returns the resulting `SyncStatus`. There is no "begin/commit" pair for the frontend to get wrong, and a failed cycle still records a status.

`SyncStatus { ran_at, outcome, detail }` with `outcome` in `Synced | NothingChanged | Created | Failed` is persisted in `app_meta`, so opening Settings after a background sync reports the real last run rather than an empty line. The same struct is what the status line renders.

### D4: Configuration is a single-row table, password in plain text

```
   sync_config (id INTEGER PRIMARY KEY CHECK (id = 1),
                url TEXT, path TEXT, username TEXT, password TEXT,
                accept_invalid_certs INTEGER NOT NULL DEFAULT 0)
```

One row, so "configured" is well-defined and there is no ordering question. The password is stored as it is used: this database already holds the full reading history in plain text, and a keychain would add a platform-specific dependency to protect one secret sitting next to far more of them. This is a deliberate acceptance, recorded here rather than left implicit.

`last_etag` and the canonical text last uploaded are stored in `app_meta` alongside the last status, keyed per configured remote path — a reader who changes the path should not be told "nothing changed" because the old path's etag still matches.

Rejected: a config file next to the database (a second thing to find, back up, and document), and OS keychain storage (platform-specific, and Android has no single answer).

### D5: TLS: a per-configuration "accept invalid certificates" switch

`reqwest::ClientBuilder::danger_accept_invalid_certs(true)` when the switch is on, off by default. A dedicated client is built per sync rather than reusing the feed fetcher's client, so the relaxation cannot leak into feed fetching. Custom CA bundles were rejected: they need a file picker, storage, and reload semantics for a problem the switch already covers.

### D6: Canonical rendering, and one ordering fix

"Did anything change" compares canonical text. That requires both renderers to agree, and they currently do not: `project` orders marks by `day, feed_url, guid`, while `merge` builds them from a `BTreeMap` keyed by `(feed, guid)`. Both are deterministic, but they are not the same order, so an export and a merged rendering of the same state would differ and the comparison would always say "changed".

`project` is changed to order marks by `feed_url, guid` — the same order `merge` produces — and a test asserts that exporting a library, merging that document with itself, and rendering again gives byte-identical text.

No `revision` counter is added. The earlier change left that as an open question; canonical text comparison answers "is anything different" exactly, on a document measured in tens of kilobytes, without a field that has to be bumped exactly when content changed and without a second merge rule (`revision = max(a, b)` makes two installations disagree forever after independent edits).

### D7: Change detection uses `If-None-Match`, with `ETag` treated as opaque

The `ETag` returned by the last GET is stored and sent as `If-None-Match`; a `304` ends the cycle without a body. `Last-Modified` is not used for decisions: the sync's own write updates it, and clock skew across machines makes it a hint at best.

A `304` ends the *download*, not the cycle: the local library may hold changes the
server has never seen, so after a 304 the cycle still renders the local document
and compares it against the text last uploaded. Missing that is how a second
installation's changes would never leave the machine until the server happened
to change as well.

If the server does not return an `ETag`, the cycle still works — the `If-Match` on the upload then uses the `ETag` from this cycle's GET, or falls back to an unconditional `PUT` when none was provided, recording that decision in the status detail so a reader on such a server understands why a concurrent write can be lost.

### D8: Startup and focus triggers live in `App.vue`, the throttle and the guard in the backend

- Startup: after the first `reload()` completes, so the sync's effects are reflected by the reload that follows it rather than racing it.
- Focus: `window.addEventListener("focus")`, calling the same command; the backend's throttle makes extra calls free.
- "Sync now": the same command, forced.

All three paths call one `syncNow(force?)` helper in the frontend, and every one of them refreshes the sidebar, counts, and list afterwards — a sync that applied remote changes must not leave a stale UI.

## Risks / Trade-offs

- **A server without `ETag` or `If-Match` support** → the cycle falls back to unconditional writes and the status detail says so. Converges on the next sync, may lose one concurrent write. Accepted; the alternative is refusing to work with such servers.
- **Two installations syncing in the same second, repeatedly** → each retry re-merges, so the merge itself never loses data; only a write that is overwritten between two retries in a row could. Bounded at three attempts, then reported rather than silently dropped.
- **A large reading history means a slow first sync** → the document is bounded by the seven-day read window, stars, and tombstones; tens to hundreds of kilobytes. No paging is needed.
- **Password in the database** → accepted deliberately (D4).
- **The focus trigger firing during a fleet of unrelated window switches** → backend throttle of five minutes; the guard drops overlapping runs.
- **`project` order change (D6)** → an existing exported document and a newly exported one may order marks differently. Harmless: the document is keyed by identity, not by position, and nothing depends on the order.
- **Self-signed certificates accepted** → only for the configured sync client, never for feed fetching.

## Migration Plan

Additive only, applied on open beside the existing idempotent migrations:

1. Create `sync_config` with its single-row constraint.
2. No new columns on existing tables; the last-etag, last-upload, and last-status records live in `app_meta`.
3. An existing database simply has no configured connection, so nothing runs until the reader fills the form in.

Rollback: an older build ignores the new table and the `app_meta` keys. No existing row changes.

## Open Questions

- Whether the sync should also run when the app is brought back to the foreground on Android by a lifecycle event other than `focus`; deferrable, and observable during implementation.
