## Context

See proposal.md - Why.

Current state that constrains the approach:

- `db::list_entries` builds a `WHERE` clause from a view (`all` / `unread`) and
  a scope (`all` / `feed` / `folder` / `starred`), then runs an unbounded
  `SELECT ... ORDER BY published_at DESC, id DESC`. Every returned row includes
  `content_html`.
- `App.vue` holds the whole result in one ref and re-fetches it on selection
  change, tab change, every refresh path, feed removal, and settings changes.
- `selectedEntry` is a computed `entries.find(...)` over that array, so the
  reading pane implicitly depends on the entry being present in the loaded
  list.
- `entries.published_at` is nullable and some feeds omit a publication date.
  There is no `created_at` column, so there is no stored arrival timestamp to
  fall back on.
- The three existing indexes all key on a scope column first and raw
  `published_at` second; none includes `id`, and none covers the global
  "All entries" ordering.
- Sidebar counts come from `entry_counts` (a global `COUNT(*)`), which is
  correct for badges and must not be repurposed for per-view totals.
- The app has no settings persistence of any kind.

A first implementation of this change shipped previous/next paging plus a
"new entries above" banner. The banner was wrong in a way that revealed the
paging model itself was the wrong interaction: it counted every row above the
current position, so opening the second page immediately reported the first
page — the rows the reader had just finished — as 50 new entries. Paging
creates a boundary that hides everything above page one, and a boundary cannot
be told apart from arrival time by position alone. That revision is what this
design replaces.

## Goals / Non-Goals

**Goals:**

- Bound every entry-list query and every list render to a batch of 50, and
  never load the full result set up front.
- Keep loaded rows stable and gap-free as more arrive and as feeds refresh.
- Keep the reading pane alive while the list grows.
- Reduce the list IPC payload from article bodies to row metadata.
- Give the reader an honest sense of how much they have loaded and how much
  matches.

**Non-Goals:**

- Notifying the reader about entries that arrived while they were scrolled
  away. The definition of "new" is a design problem of its own and is
  deferred to a dedicated change; a continuous list has no boundary hiding
  new rows anyway, so scrolling up reveals them.
- Jump-to-page navigation or page-number buttons.
- Virtualized rendering of the loaded rows (50-at-a-time growth with
  body-free rows does not need it yet).
- Configurable batch size or any settings persistence layer.
- Pruning, archiving, or retention policies for old entries.
- Changing sidebar unread/total badge semantics.
- Keyboard navigation or scroll-position restoration.

## Decisions

### D1: Keyset cursor for appending, no page boundaries

**Decision:** Each batch continues from the ordering position of the last
loaded row — `(sort key, id)` — and rows are appended to one accumulated
list. The frontend holds a single `after` cursor (plus a `hasMore` flag),
not a stack of page boundaries.

**Why:** Keyset still buys what offset cannot: batches are content-delimited,
so a refresh that inserts rows above cannot shift, duplicate, or drop the rows
already on screen, and each batch costs one `LIMIT`-bounded read regardless of
how deep the reader has scrolled.

The navigation chrome around it, however, changed. Previous/next pages forced
a choice between an invisible region above page one (fixed only by a banner)
and an unsound definition of "new"; a continuous list has no such region.
Scrolling up simply walks back through what was loaded.

**Alternatives considered:**

- *`LIMIT/OFFSET` with page numbers* — supports jumping but drifts on every
  refresh, since a position is not a boundary.
- *Previous/next keyset pages* — stable, but the page boundary creates the
  blind spot that motivated the banner, and the banner's own definition of
  "new" could not be made sound from position alone. Rejected after user
  feedback that the interaction was odd.
- *Infinite scroll that replaces rather than appends* — loses the ability to
  scroll back, which is the main reason readers prefer it to paging.

**Consequence:** The reader can scroll back through loaded rows but cannot
jump to an arbitrary depth without scrolling (or reloading, which starts at
the newest batch).

### D2: Arrival timestamp as the tie-breaker base, not a sentinel or an expression

**Decision:** Add `entries.created_at`, backfill existing rows with
`datetime('now')`, set it explicitly in `upsert_entry`'s `INSERT`, and never
touch it in `ON CONFLICT DO UPDATE`. The order key is
`COALESCE(published_at, created_at)`.

**Why:** The cursor needs a *total* order; a `NULL` published time breaks
tuple comparison because `(NULL, id) < (?, ?)` evaluates to `NULL` and would
silently drop every undated entry once the reader scrolls past the first
batch. Storing a real arrival time gives undated entries a meaningful,
immutable position, and it is generally useful beyond loading.

**Alternatives considered:**

- *`COALESCE(published_at, '')`* — no schema change, but relies on `''`
  sorting below every real timestamp and keeps `created_at` unavailable for
  other uses.
- *Backfill a `1970-01-01` sentinel into `published_at`* — keeps plain-column
  ordering and existing indexes usable, but leaks a fake date into the API and
  requires the UI to special-case it.
- *Store the coalesced value into `published_at` at write time* — simplest
  ordering and index story, but destroys the "this entry has no publication
  date" fact.

**Accepted cost:** Existing undated entries are backfilled with the migration
moment and therefore jump to the top of the list instead of sitting at the
bottom. Also, `ALTER TABLE ... ADD COLUMN` cannot use a function as its
default, so the backfill must be a separate `UPDATE`.

### D3: Expression index for the global ordering

**Decision:** Add
`CREATE INDEX idx_entries_sort ON entries(COALESCE(published_at, created_at) DESC, id DESC)`.
Leave the three existing scope indexes in place.

**Why:** The order key is an expression, so the existing indexes (which store
raw `published_at`) can no longer satisfy `ORDER BY`. SQLite supports indexes
on deterministic expressions, and a `WHERE` clause containing the identical
expression can use it. The three existing indexes still serve their `WHERE
feed_id = ?` / `is_read = ?` / `is_starred = ?` filtering role; sorting the
already-filtered subset of a single feed or folder is cheap, so only the global
path needs the new index.

**Alternative considered:** Re-keying the existing indexes on the expression —
rejected as churn that would degrade per-feed lookups, which are currently the
best-served path.

### D4: Split list metadata from entry content

**Decision:** `list_entries` returns a new `EntryListItem` without
`content_html`; a new `get_entry(entry_id)` returns one full `Entry` for the
reading pane.

**Why:** The list displays only a title, a date, and a star, yet ships every
entry's body. Even one batch of 50 entries is roughly 1 MB of article HTML
versus a few kilobytes of titles, and a scroll-loaded list holds several
batches at once. The reading pane needs an on-demand fetch anyway once it can
outlive the loaded rows, so the two changes are the same change.

**Trade-off:** Opening an entry costs one extra IPC round trip.

### D5: The reading pane owns its own entry copy

**Decision:** Replace the computed `entries.find(...)` with a separately loaded
`selectedEntry` ref, populated by `get_entry` on selection and held while
batches load; cleared only when `get_entry` reports the entry is gone (or its
feed is removed).

**Why:** Loading more deliberately appends rows around the open entry, so a
list-derived selection is unnecessary coupling.

**Consequence requiring care:** read/star toggles must update *both* the list
row (so the unread dot and star reflect state) and the pane's copy (so the
toolbar toggle agrees), or the two panes drift apart. The pane's copy is the
source of truth for the open entry; the list row is the source of truth while
the entry is visible.

### D6: Progress is derived; new-arrival notification is dropped

**Decision:** "Loaded" is simply the number of rows on screen, and the total
comes from a new `count_entries(view, scope)` that mirrors `list_entries`'
where-clause construction. There is no new-arrival counter: `count_newer` was
implemented and then removed with the banner.

**Why:** Every batch except possibly the last is exactly 50 rows by
construction, so the loaded count is free arithmetic. A dedicated per-view
count is necessary because `entry_counts` is global and the unread view's
total differs from it.

**The banner defect, for the record:** counting rows above the current
position cannot distinguish "arrived since I last looked" from "the batch I
just read" — scrolling one batch down reported exactly one batch of new
entries, forever. Two sound definitions exist and both are deferred:

- *Arrival-based*: remember the wall-clock time of the first load and count
  rows with `created_at` after it. Catches a backfilled old article that sorts
  below the reader's position; needs a display rule for "already visible".
- *High-water mark*: remember the ordering position of the newest row at first
  load and count rows above it. Needs no clock, but misses late-arriving old
  articles.

Neither is needed for a continuous list, where new rows are simply above the
reader and reachable by scrolling up.

**Alternative considered:** A single query returning rows plus a windowed
`COUNT(*) OVER ()` — gives the total with one round trip but forces SQLite to
materialize the count over the full filtered set for every batch, which is
worse than one cheap count on demand.

### D7: Fetch `limit + 1` and slice to detect the end

**Decision:** Request 51 rows for a batch of 50, treat `hasMore` as
`rows.length > 50`, and surface 50.

**Why:** `LIMIT 50` returning exactly 50 rows is ambiguous between "exactly 50
remain" and "more remain", which would otherwise produce a sentinel that keeps
firing into an empty list.

### D8: A refresh reloads the loaded window, not just one page

**Decision:** On refresh, re-request from the newest cursor with
`limit = max(loaded rows, 50)`, then replace the list with the returned rows
(sliced to that depth) and recompute `hasMore`.

**Why:** A refresh can only be observed through rows — new arrivals appear at
the top and the reader's loaded rows must be re-read to reflect them. Reloading
the whole loaded window in one query keeps it a single round trip; the rows are
body-free, so even a few thousand of them are cheap.

**Trade-off:** Rows that arrived above push the loaded window down by the same
amount, so the tail of a deep list can be trimmed. Browsers' scroll anchoring
keeps the reader visually in place, and anything trimmed was loaded but is
still reachable by scrolling up from the top... which is not true once it is
trimmed. This is accepted as long as deep loads stay in the low thousands; if
it becomes visible, the fix is to preserve the tail explicitly.

### D9: The list state machine is verified by tests, not by watching it

**Decision:** A `vitest` suite with `@vue/test-utils` and `jsdom` exercises
`App.vue`'s load/append/reset logic and `EntryList.vue`'s sentinel against a
fake Tauri backend that reproduces `db.rs`'s paging contract exactly (sort
key, keyset cursor, `limit + 1`).

**Why:** The behaviour this change introduces — cursor continuity, the
empty-batch stop, discarding a batch that was in flight when the reader
changed scope, refreshing the loaded window — is precisely the behaviour that
is easy to get subtly wrong and unpleasant to debug by hand in a GUI. The fake
backend is worth calling out: if it simplified the contract, the tests would
prove only that the frontend agrees with a fiction.

**Trade-off:** jsdom reports a zero `getBoundingClientRect` for every element,
which makes the sentinel look permanently on screen and would chain batches to
the end of the list; the geometry is therefore stubbed, with one test for each
branch (still visible after a batch -> ask again, scrolled away -> stay
quiet). Visual polish remains untested.

## Risks / Trade-offs

- **[Read/star state diverges between the list row and the open entry]** →
  D5; every toggle updates both objects through one shared path, and tests
  cover toggling an entry that is open but off the loaded rows.
- **[`created_at` is corrupted by a re-storing feed, invalidating cursors]** →
  `upsert_entry`'s conflict branch must not mention `created_at`; a test
  asserts the value survives a re-fetch unchanged.
- **[Undated entries jump to the top on migration]** → accepted in D2 and
  disclosed; it is a one-time reordering of entries that had no date to order
  them by anyway.
- **[The loaded list grows without bound as the reader scrolls]** → each row
  is body-free (title/date/star only) and batches are 50, so thousands of rows
  are far cheaper than the previous full-body payload; virtualization is the
  escalation path and is explicitly a non-goal here.
- **[A refresh can trim the tail of a very deep load]** → accepted in D8;
  bounded by how far a reader has scrolled in one session.
- **[An append returns nothing because its rows were deleted]** → treat an
  empty batch as "no more" and stop the sentinel instead of retrying.
- **[Existing tests break on the `list_entries` signature change]** →
  `fetch.rs`, `reader.rs`, `opml.rs` and much of `db.rs`'s suite call it
  directly; the migration and signature change must land with those call sites
  in the same change.
- **[Two count queries per load add latency]** → the total is one indexed
  `COUNT(*)` over a filtered set, run once per load.

## Migration Plan

1. Schema: add `entries.created_at`, backfill with `datetime('now')` where
   `NULL`, create `idx_entries_sort`. All statements are idempotent and run on
   every open, matching the existing `COLUMN_MIGRATIONS` / `POST_MIGRATION`
   pattern, so a rollback to the previous binary is safe: the older code never
   reads `created_at` and simply ignores the extra column and index.
2. Backend: change `list_entries` and add `count_entries`, `get_entry`;
   register commands; fix in-repo call sites and tests. The IPC signature
   change is breaking, so frontend and backend land in the same commit.
3. Frontend: types first (`EntryListItem`, `Cursor`), then `App.vue` state,
   then the `EntryList.vue` progress footer and scroll sentinel.

Rollback = revert the commit; the column and index are additive and harmless.

## Open Questions

- Whether a new-arrival notification should be revived, and under which
  definition of "new" (arrival-based vs high-water mark). Deferred to a
  dedicated change; the two candidates are recorded in D6.
- Whether the progress line should also report how many batches remain, or
  whether "loaded / total" is enough.
- Whether deep loads eventually need virtualization or an explicit cap on
  retained rows; deferred until measurements exist.
