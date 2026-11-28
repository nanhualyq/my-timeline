## Context

See `proposal.md` for motivation and `specs/webdav-sync/spec.md` for the required behavior.

The relevant current state:

- `SyncOutcome` is decided inside the read/merge/write cycle (`src-tauri/src/sync.rs`): `unchanged` short-circuits to `NothingChanged` as soon as the merged rendering equals the document just read *or* the one last uploaded. A download-only sync therefore lands on `NothingChanged` even though `import` applied the remote records to the database.
- `SyncStatus` is serialized into `meta` (`remote_status_json`, keyed by document URL), so its shape is both an IPC payload and stored state that outlives a single run.
- `SyncEngine::sync` replays a previously recorded status in two cases: a focus-driven trigger inside the five-minute throttle, and a trigger arriving while a cycle is already running.
- The frontend decides "the reader must be refreshed" from `outcome` alone (`appliedChanges` in `src/lib/background-sync.ts`), and the Settings dialog repeats that decision inline for "Sync now". The backend knows the truth; the frontend only guesses it.

Constraint: entry bodies are not shared, so a refreshed view can legitimately show a synced subscription with no entries. Refreshing the view must therefore not be conflated with fetching feeds.

## Goals / Non-Goals

**Goals:**

- One authoritative signal for "the local library changed", produced where the merge happens and consumed by every refresh decision.
- The signal survives an upgrade: statuses recorded by earlier builds stay readable, and both directions (old status read by new build, new status read by an older build) remain safe.
- No change to when syncs run, what the document contains, or what the status line says beyond the added fact.

**Non-Goals:**

- Fetching entries for newly synced subscriptions during the sync (stays with the existing refresh actions).
- Notifications, toasts, or any new UI surface; the existing status line remains the only report.
- Reworking merge, counters, or tombstones.

## Decisions

### Decision: add `applied: bool` to `SyncStatus` instead of a new outcome variant

A new variant (`downloaded`) would overload an enum that answers "what happened on the server": `synced`/`created`/`nothing_changed`/`failed` are about the document, and the dialog wording, stored statuses, and throttle replay all depend on that meaning. Whether the *local* library changed is an independent axis — a download-only sync is `nothing_changed` + applied, a local-only upload is `synced` + not applied — so it gets an independent field.

Alternatives considered: deriving it in the frontend from `detail` text (fragile string parsing); a second command that re-reads the library (a second round trip and a race with the next sync); making the cycle always upload (extra network writes against the "no work when nothing changed" requirement).

Field name `applied`, `#[serde(default)]`, so a status JSON written by an earlier build deserializes with `applied = false`, and an older build reading a new status ignores the unknown field (serde ignores unknown fields by default). No database migration: the stored JSON simply gains a key.

### Decision: the cycle reports the merge result, and it accumulates across attempts

`cycle` already computes an `ImportSummary` per attempt; `nothing_applied(&summary)` says whether this attempt changed the library. Every `SyncStatus` created after a merge point takes the accumulated answer (logical OR over attempts), and statuses created before any merge (transport failures, unreadable documents) report `applied = false`.

Accumulation matters because a rejected write re-reads and re-merges: the second attempt imports a document it already applied and reports zero changes, so without accumulation the library change from the first attempt would be lost.

Constructor shape: `SyncStatus::now(outcome, detail)` keeps meaning "nothing was applied" and a sibling `SyncStatus::applied(outcome, detail)` (or an `applied` builder) is used where a summary exists. Keeping `now` unchanged means the pre-merge failure paths stay untouched and the diff stays small.

### Decision: the frontend refreshes on `applied`, with the old outcomes as a fallback

`appliedChanges(status)` becomes `status.applied === true || outcome is synced/created`. The outcome fallback exists only for a status recorded by an earlier build, where `applied` defaults to `false` but the old meaning of `synced`/`created` was "refresh". The Settings dialog stops duplicating the rule and calls the same helper, so a download-only manual sync refreshes the view behind the open dialog (the dialog's own status line already shows the applied counts).

Consequence: a throttled or dropped trigger replays the recorded status, and if that status says `applied`, the background path refreshes again. That reload is a local database read that preserves selection, filter, and the loaded window, so a repeated refresh is cheap and invisible; tracking "already refreshed this status" would add state to solve a non-problem.

### Decision: no refresh-vs-fetch coupling

`reload()` already re-reads feeds, folders, counts, the loaded entry window, and the open entry. Newly imported subscriptions appear there immediately, with the entries this installation already holds (usually none). That satisfies the requirement without adding a fetch to the sync path, which would turn a quiet background sync into a burst of network requests against feeds the reader may not care about yet.

## Risks / Trade-offs

- [A failed sync that applied changes now refreshes the view] → Intended: the reader must not keep reading stale read/star state because the upload failed. The status line still reports the failure and its reason.
- [Repeated idempotent reloads on focus events inside the throttle] → Accepted; a reload is local-only and preserves the reader's position. Revisit only if profiling shows it.
- [Stored status JSON shape widens] → Compatible in both directions (`serde(default)` plus unknown-field tolerance); no migration step, and a downgrade does not corrupt the stored status beyond losing the added fact.
- [A refresh can race the reader's own navigation] → `reload()` already discards in-flight windows from a previous generation and re-reads the open entry by id, so the new trigger inherits the existing safety rather than adding a second path.
