## Context

See `proposal.md` for why. The constraints that shape the approach come from the code as it stands:

- A refresh is one backend command that returns only when every feed has finished (`refresh_all` in `src-tauri/src/reader.rs`), so there is nothing to observe while it runs. The frontend has no way to learn per-feed progress short of changing that shape.
- Nothing in the application persists a preference today. `PAGE_SIZE` in `src/App.vue` says so explicitly: the app has no settings storage. The `app_meta(key, value)` table exists and is used for `device_id` and the sync counter; nothing in `app_meta` is exported to the sync document.
- The only event-free IPC is `invoke`/`Promise`. Tauri events are used nowhere in the repository, so introducing them is a new pattern, not an extension of one.
- `src-tauri/src/fetch.rs` builds a fresh `reqwest::Client` per request, sends no conditional headers, and `store_feed` writes each entry in its own implicit transaction.
- Every trigger already lives in the frontend (`App.vue`: button, `r` keystroke, sidebar menus, foreground return), so a runner in the frontend does not move orchestration across the process boundary — it consolidates orchestration that is already here.

## Goals / Non-Goals

**Goals:**

- One refresh runner that every trigger shares, with per-feed progress, cancellation, single-flight, host politeness, and a per-feed result the panel can render.
- A schedule whose semantics are "each feed is fresh for the interval", not "run a batch every interval".
- Refresh feedback that never blocks the reader and never reports a run as silent success or silent failure.
- One concurrency policy. Two implementations of the same policy is the failure mode this design is trying to remove.

**Non-Goals:**

- A background scheduler in Rust. The interval granularity is minutes; Chromium's background timer clamping only matters at sub-minute granularity, and an Android process suspended by the OS pauses a Rust timer just as effectively as a JavaScript one.
- Conditional requests (ETag/Last-Modified) or a shared `reqwest::Client`. Both are worth doing and both are independent of this change's shape; folding them in would mix a transport optimisation into an orchestration redesign. They are listed as follow-ups.
- Registrable-domain grouping via a public suffix list. Feeds are grouped by hostname; see Risks.
- Per-feed custom intervals, retry scheduling outside a live run, and any notion of a background service.

## Decisions

### The runner lives in the frontend, not behind an event stream

Two shapes were possible:

```
A. Backend emits events                 B. Frontend orchestrates
   refresh_all() -> app.emit(...)          for target of queue:
   listener -> panel                          await refreshFeed(id)
                                              onProgress(...)
```

Chosen: **B**. The frontend already owns every trigger and already knows the feed list (`feeds[]` carries `id`, `url`, `last_fetched_at`, `last_error`), so grouping, due-eligibility, and scope resolution need no new IPC at all. Progress becomes a counter in the same module that owns the loop, cancellation is "stop pulling from the queue", and the whole thing is testable with the existing `fakeBackend` harness in Vitest. Shape A would need an event protocol, listener lifecycle handling, and a second concurrency implementation in Rust while still leaving the frontend to reconcile its `busy` flag with events it did not initiate.

Consequence: `refresh_all` and `refresh_folder` are deleted rather than kept as a second path, together with `refresh_targets`, `MAX_CONCURRENT_FETCHES`, and `RefreshOutcome`. A per-feed command is the only refresh primitive.

### `refresh_feed` returns a result, not `()`

```rust
pub struct FeedRefresh {
    pub feed_id: i64,
    pub error: Option<String>,
    pub new_entries: u32,
    pub http_status: Option<u16>,
}
```

The runner needs three facts it cannot derive client-side: whether the feed failed, how many entries are new, and whether a failure was a rate-limit refusal rather than a broken feed. All three travel on the same return value, so one command change serves the panel, the summary, and the host backoff.

Deriving `new_entries`:

- Chosen: `SELECT COUNT(*) FROM entries WHERE feed_id = ?` before and after the store, inside the store's transaction. `entries(feed_id, ...)` is indexed, and the statement runs twice per feed.
- Rejected: `INSERT ... ON CONFLICT DO UPDATE ... RETURNING (created_at = ?marker)`. It works in one statement, but it forces `created_at` to be written from Rust with a caller-chosen value, and `sort_key` is `COALESCE(published_at, created_at)` compared as strings while `created_at` is `datetime('now')` (`'YYYY-MM-DD HH:MM:SS'`) and `published_at` is RFC3339. Handing `created_at` a third spelling would quietly reorder entries. Not worth one saved statement.
- Rejected: `changes()`. It reports 1 for both the insert and the update branch of the upsert.

### The whole feed store runs in one transaction

`store_feed` currently executes the feed upsert, then one `upsert_entry` per entry, then `apply_marks_to_entries`, each in its own implicit transaction. A 30-entry feed is 32 commits per fetch. Wrapping the three steps in one transaction removes that, makes the before/after count exact, and makes a feed's entries land atomically — a reader reloading mid-store cannot observe half a feed.

The transaction is taken on the shared connection inside `fetch_and_store`, so the parse (which is the slow part) still happens outside the lock.

### The scheduler is two constraints, not one pool

```
targets ──► group by hostname ──► round-robin interleave ──► dispatch
                                                              │
                                        global semaphore (5, 3 on touch)
                                        per-host: one in flight + 300 ms gap
```

- The global semaphore bounds total parallel requests.
- The per-host chain is a promise tail per hostname: each target waits for its host's previous request plus the gap. This is the whole rate-limit defence and it needs no timer bookkeeping.
- Interleaving is not cosmetic. Dispatching in hostname-grouped order sends the first `limit` requests to the same host, which collapses effective parallelism to one. Round-robin draw from the host groups keeps the first batch spread across distinct hosts.

Concurrency is 5 where the primary pointer is fine and 3 on a touch layout, because a cellular connection gains nothing from five sockets and loses latency to them.

`300 ms` between requests to the same host is a politeness constant, not a rate-limit contract. It is cheap, invisible at this scale, and covers the common case of one site with several feeds. The real signal is the response; see below.

### Rate limiting is handled from the response, not only preventively

Preventive pacing only reduces the odds. When a fetch fails with 429 or 503, the runner:

1. Records the failure for that feed as a *deferred* result tied to its host, not as a per-feed failure.
2. Holds the rest of that host's queue behind a delay that grows with each refusal (60 s, then 300 s).
3. Continues every other host unchanged.
4. Retries the deferred feeds once the delay elapses, if the run is still in flight.

The status has to come from the backend, because today only a formatted string (`"HTTP 429"`) crosses the boundary. Parsing that string would pin the shape of an error message as an API; `http_status` in `FeedRefresh` states it.

### The schedule is a 60-second tick plus due-eligibility

Rather than "every N minutes, fetch everything":

```
every 60 s: for feed in library:
              due = now - last_fetched_at >= interval + jitter(feed)
            if any due: runRefresh(scope = all, reason = auto)
```

The tick itself is nearly free: with nothing due it does no IPC. Fetch load is spread by the reader's own usage pattern (the `last_fetched_at` values are already staggered), so no artificial spreading is needed beyond a per-feed jitter offset of up to a minute derived from the feed id, which only prevents feeds added in one OPML import from sharing a due time forever.

This also settles the interaction between triggers. A manual refresh at 12:00 makes every touched feed ineligible until 12:30, so the schedule does not immediately duplicate work that just happened. Foreground return joins the same rule: it runs a due-eligible check instead of the current unconditional refresh, which is what `resumeRefresh` does today.

Visibility: the tick returns immediately while `document.hidden`, and a `visibilitychange` to visible runs a check. `resumeRefresh`'s existing 60-second hidden threshold is kept as the definition of "a return", so opening the app switcher does not trigger work.

Persistence: one `app_meta` row, `refresh_settings`, holding JSON (`{"enabled": bool, "interval_minutes": number}`), read and written by two small commands. Not part of the sync document: two devices have different networks, batteries, and reading habits, and syncing this would only make them fight.

### Single-flight is per feed within a run, not a global lock

The runner keeps the set of feed ids it has dispatched or completed. A trigger arriving mid-run:

- resolves its own target list,
- removes the feeds already in the set,
- adds the remainder to the live queue, or does nothing when nothing remains.

Consequences that fall out for free: mashing `r`, pulling during a scheduled run, and a foreground return during either are all safe; a narrow trigger during a broad run reports nothing new; a broad trigger during a narrow run extends it.

A global "already running, ignore" flag was rejected: a reader who pulls during a scheduled run would get no refresh of their own scope, which is the case most likely to happen in practice.

### The panel is a notice, not a dialog

`NoticeStack` (new) is one fixed bottom region owning both `RefreshPanel` and the existing `UndoBar`, so a batch mark landing mid-run stacks instead of overlapping. z-index stays below dialogs (25 today) and above panes and drawer.

Panel lifecycle:

- Shown only after the run has been in flight for ~400 ms, so a two-feed refresh does not flash. A run that finishes sooner reports only its summary.
- Running: completed count, the finished feeds with their outcome, failures with their reason, a Stop action.
- Finished, no failures: summary ("24 feeds · 12 new"), withdraws after ~4 s.
- Finished with failures: summary stays, lists failures, offers Retry failed and dismiss.

Announcements use `role="status"` / `aria-live="polite"` on the summary line only, never on the live progress counter — a counter announced on every feed would flood assistive technology.

### The pull is driven by touch events, not pointer events

Found on a device, not in a test: inside a scroll port the browser arbitrates a
vertical pan itself, and once it does, it sends `pointercancel` to anything
following the pointer — so a pointer-driven pull never accumulates the travel it
needs and the list rubber-bands instead. Claiming the gesture back requires a
non-passive `touchmove` that calls `preventDefault`, which is only done once the
pull is actually claimed (a sideways drag, an upward drag, or a press on a
scrolled list leaves scrolling completely alone).

Consequence for tests: the pull's integration tests dispatch touch events.
jsdom does not arbitrate gestures at all, so no pointer-based test can tell a
working pull from one that a real browser cancels.

### Stop is acknowledged before it takes effect

An in-flight request cannot be recalled, so a stop that only becomes visible
when the last request returns (up to the 20 s timeout) looks like a dead
button. The panel therefore switches to "Stopping…" and withdraws the stop
control the moment the reader asks, and the summary reports the stop when the
run actually ends.

### The pull gesture is a second recognizer, not a mode of the first

`src/lib/gestures.ts` holds a horizontal machine whose whole point is locking direction before claiming. The pull is a separate recognizer with one extra precondition the horizontal machine does not have: the list's `scrollTop` must be 0 when the press lands, and the movement must lock vertical-downward. Sharing one machine would mean teaching it about scroll containers, which is exactly the coupling the current file avoids.

The pull attaches to the list's scroll port (`.entries`), not the window, because the list scrolls internally. It applies resistance (`dy * 0.5`, capped) and commits past a threshold; before the threshold it springs back and refreshes nothing. Committing starts a run of the current scope (feed → that feed, folder → that folder, starred → the feeds holding starred entries, all → every feed) and hands off to the panel: the pull's own affordance ends where the panel begins, so there is no second progress indicator to keep in sync.

Row-menu long press and horizontal drags are unaffected: a stationary hold never passes the pull's slop, and a horizontal movement never locks downward.

### Settings surface

The Settings dialog gains a Refresh section in its existing style: a switch, a select of fixed intervals (15/30/60/180 minutes), and two read-only lines for the last scheduled refresh and the next due time. Both controls persist on change and report in the dialog, matching how the WebDAV section already reports its own saves.

## Risks / Trade-offs

- **[Hostname grouping misses sibling subdomains]** `blog.example.com` and `feeds.example.com` are two hosts, so both may be hit at once. → Accepted: the cost is one extra parallel request, and the 429 handling catches the pathological case. A public suffix list would add a dependency for a rare benefit.
- **[Frontend timers are clamped or paused in the background]** A scheduled check may be late or skipped while the app is hidden or the OS suspends it. → Accepted by design: the schedule deliberately does not run hidden, and the visibility check plus due-eligibility makes the catch-up immediate on return. Nothing is lost but freshness during the hidden period.
- **[Deleting the backend commands is a breaking API change]** Anything calling `refresh_all`/`refresh_folder` breaks. → This repository is the only caller; both call sites are rewritten in the same change, and the Rust tests for the deleted paths are removed rather than left failing.
- **[Per-feed IPC has a cost]** A 100-feed run makes 100 `invoke` round trips instead of one. → Measured against the network fetch each one triggers (hundreds of ms at best), the IPC cost is noise, and it buys cancellation and progress that a single call cannot express.
- **[The pull cannot be verified by tests alone]** The browser's gesture arbitration is invisible to jsdom, and the first implementation passed every test while doing nothing on a phone. → The gesture was walked on a device (pull, stop, retry) and the touch-event decision is written down above; any future change to it needs the same walk.
- **[Long tail from one host]** A host with many feeds whose requests each hit the 20 s timeout serialises into minutes. → Stop halts dispatch, per-request timeouts bound each attempt, and the 429/503 path short-circuits the failures cheaply instead of letting every deferred feed time out on its own.
- **[Interface-string coupling removed, but the failure text stays]** The panel shows the stored error string. It is prose, not a taxonomy. → Acceptable: `http_status` carries the machine-readable part; the string explains it to the reader.
- **[`store_feed` in one transaction lengthens the write lock]** A large feed holds the connection for the duration of its writes instead of yielding between entries. → Accepted and intended: the parse is the slow part and stays outside the lock; the write burst is short and the alternative is a partially stored feed visible to a concurrent reload.

## Migration Plan

No schema migration: `refresh_settings` is a new `app_meta` key written on first save, and its absence reads as "schedule off, interval 30 minutes". Databases from earlier versions therefore keep behaving as today (manual only) until the reader opts in. No sync-document format change, so an older installation can still consume the document.

Deployment is a single application release. Rollback is a rebuild from the previous tag; nothing persisted by this change prevents an older build from opening the library, because the only new state is one ignored key.

## Open Questions

- Whether the wide layout should also accept the pull gesture with a mouse drag. Deferred: the button is present there, and teaching a mouse drag that competes with text selection is a separate question.
- Whether the retry action should also reset a feed's failure backoff. Deferred: backoff only exists inside a live run today, so the question does not arise yet.
