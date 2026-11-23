## 1. Schema and ordering foundation

- [x] 1.1 Add the `created_at TEXT` column migration to `db.rs` (`COLUMN_MIGRATIONS`) plus a backfill `UPDATE entries SET created_at = datetime('now') WHERE created_at IS NULL`, and verify `cargo test` in `src-tauri` still passes with the schema tests re-running `init_schema` idempotently on both a fresh and an already-migrated database.
- [x] 1.2 Create the expression index `idx_entries_sort ON entries(COALESCE(published_at, created_at) DESC, id DESC)` in `POST_MIGRATION`, and verify a test opens a database twice and confirms the index exists exactly once after repeated initialization.
- [x] 1.3 Make `upsert_entry` set `created_at` explicitly on `INSERT` and omit it from the `ON CONFLICT DO UPDATE` branch, and verify with a test that re-storing the same feed leaves the original `created_at` unchanged.
- [x] 1.4 Add a test that an entry with `published_at IS NULL` is ordered by `created_at`, sits above entries published earlier, and stays reachable when a later batch is requested (i.e. it is returned by a cursor query rather than filtered out).

## 2. Backend batch queries

- [x] 2.1 Introduce an `EntryListItem` DTO without `content_html` and change `db::list_entries` to accept an `after` cursor and a `limit`, ordering by `COALESCE(published_at, created_at) DESC, id DESC` with a tuple comparison when a cursor is supplied; verify with a test that two consecutive 50-row batches contain no overlapping and no skipped entries among 120 seeded rows.
- [x] 2.2 Make the query request `limit + 1` internally so callers can derive `hasMore` from receiving more than `limit` rows, and verify with a test that seeding exactly 50 rows yields 50 returned rows and no "more" signal, while 51 rows yields the extra row.
- [x] 2.3 Add `db::count_entries(view, scope)` reusing the same condition construction as `list_entries`, and verify with tests covering the `all`/`unread` views and the `all`/`feed`/`folder`/`starred` scopes that the totals match seeded expectations.
- [x] 2.4 (Superseded by 7.1 — kept for history) Add `db::count_newer(cursor, view, scope)` counting entries strictly newer than the cursor, with tests for entries above, below, and outside the scope. It served the new-arrival banner, which was removed once the banner's definition of "new" was shown to be unsound.
- [x] 2.5 Add `db::get_entry(entry_id)` returning the full `Entry` including `content_html`, and verify with tests that it returns the stored body for an existing id and `None` for an unknown id.

## 3. Command registration and in-repo call sites

- [x] 3.1 Register `count_entries`, `count_newer`, and `get_entry` alongside the changed `list_entries` in `lib.rs`'s `generate_handler!` (`count_newer` is removed again in 7.1), and verify `cargo build` in `src-tauri` compiles.
- [x] 3.2 Update the direct `list_entries` call sites in `fetch.rs`, `reader.rs`, `opml.rs`, and `db.rs` tests to the new signature and result type, and verify `cargo test` in `src-tauri` passes in full.

## 4. Frontend API layer

- [x] 4.1 Add the `EntryListItem` interface (no `content_html`) and a `Cursor` type to `src/api.ts`, and verify `npm run check` reports no type errors.
- [x] 4.2 Update `listEntries` to send the cursor and batch size and add `countEntries`, `countNewer`, and `getEntry` (`countNewer` is removed again in 7.1), and verify `npm run check` passes.

## 5. Frontend list state (scroll loading)

- [x] 5.1 Replace the `boundaries` stack with a single append cursor plus an accumulated rows array: the first batch starts from `null`, each later batch continues from the last loaded row's `sort_key`/`id`, and rows are appended rather than replacing the list. Verify by running the app that scrolling loads more than 100 entries with no repeats and no gaps. (Verified automatically: `app.spec.ts` asserts 100 loaded rows are exactly ids 120..21 — no repeats, no gaps — and that the second `list_entries` call carries the first batch's last row as its cursor.)
- [x] 5.2 Load the selected entry through `getEntry` into a ref that is held while further batches load, and clear it only when `getEntry` returns `null` or the entry's feed is removed. Verify that opening an entry and then scrolling keeps the reading pane open, while removing that entry's feed clears it.
- [x] 5.3 Route read and star toggles through one `patchEntry` path that updates both the list row and the pane's copy. Verify that marking an open entry read updates the list's unread dot, the pane's toggle, and the sidebar count together, including for an entry that is open but not among the loaded rows.
- [x] 5.4 Split `reload()` into "start over" (selection, folder/feed/global view, and all/unread tab changes clear the loaded rows and load the first batch) and "reload the loaded window" (refresh paths re-request from the newest cursor with `limit = max(loaded rows, 50)`, keeping `hasMore` accurate). Verify that a refresh after scrolling keeps the loaded rows with any new entries above them, and that selecting another feed from a deep load starts at that feed's newest batch with no leftover rows. (Verified automatically: the refresh test asserts `limit = 100, after = null` with 100 rows still on screen, and the generation-guard test holds a batch in flight across a scope change and asserts no stale rows leak in.)
- [x] 5.5 Handle an empty result: an append that returns no rows stops loading (`hasMore = false`) instead of retrying, and a reload whose window comes back empty falls back to the first batch. Verify by deleting the loaded entries that the reader is returned to a populated list rather than a blank one. (Verified automatically: emptying the backend between batches leaves the 50 loaded rows on screen, sets `hasMore = false`, and issues no further `list_entries` call.)
- [x] 5.6 Compute the progress line as `entries.length` (loaded rows) and the total from `countEntries` for the active view and scope, with no new-arrival counter. Verify the unread view shows the unread total rather than the global total. (Verified automatically: 120 seeded / 40 read -> total 120 on All, 80 on Unread; per-view totals also covered by `count_entries_matches_each_filtered_view_and_scope`.)

## 6. Entry list UI

- [x] 6.1 Change `EntryList.vue`'s prop type from `Entry[]` to `EntryListItem[]`, and verify `npm run check` passes with the reading pane still receiving the full entry for the selected row.
- [x] 6.2 Rework the footer into a progress line ("N / M entries viewed") with no previous/next controls, plus an end-of-list state when `hasMore` is false. Verify the display against an empty list, a single partial batch, and a multi-batch list. (Verified automatically in `entryList.spec.ts`: the progress text, `End of list` / `Scroll for more` / `Loading…` states, and the hidden sentinel on an empty list.)
- [x] 6.3 Add a scroll sentinel at the end of the list that requests the next batch while `hasMore` is true, shows a loading state while the request is in flight, and disappears at the end of the list. Verify that scrolling past 100 entries loads batches automatically without any click. (Verified automatically: the observer is rooted on the list with a 150px lead-in, fires `loadMore` on intersection only while `hasMore`, and the post-load re-check fires again when the sentinel is still on screen and stays quiet once it has scrolled away.)

## 7. Remove the new-arrival banner (superseded design)

- [x] 7.1 Remove `db::count_newer` and its test, the `count_newer` Tauri command and its registration, `countNewer` in `src/api.ts`, the `newArrivals`/`goToNewest` state and handlers in `App.vue`, and the banner markup plus styles in `EntryList.vue`. Verify `cargo test`, `npm run check`, and `npm run build` all still pass.

## 8. Verification

- [x] 8.1 Re-run `cargo test` in `src-tauri`, `npm run check`, and `npm run build` after the scroll-loading redesign, and verify all three complete without errors.
- [x] 8.2 Walk the scenarios in `specs/entry-list-pagination/spec.md` and `specs/entry-starring/spec.md` against the running app, including the starred view scrolling with no unread filter, and verify each scenario's stated outcome. (Verification basis changed with approval: every behavioural scenario is asserted by the 23-test suite in `src/__tests__/`; the manual walkthrough of a running app is recorded as a pre-release check rather than a gate for this change.)
- [x] 8.3 Run `openspec validate paginated-entry-list --strict`, and verify the change reports no validation errors.

## 9. Automated coverage of the list state machine (approved scope expansion)

- [x] 9.1 Add `vitest`, `@vue/test-utils`, and `jsdom` as dev dependencies with a `test` script and a `vitest.config.ts` kept separate from the Tauri dev-server config, and verify `npm test` runs to completion.
- [x] 9.2 Add a fake Tauri backend that mirrors `db.rs`'s paging contract (sort key ordering, keyset cursor, `limit + 1` rows) plus hold/release gating for in-flight calls, and an `IntersectionObserver` stub, under `src/__tests__/fakeBackend.ts`, and verify `npm run check` type-checks them under the project's strict settings.
- [x] 9.3 Cover the App list state machine in `src/__tests__/app.spec.ts` — bounded first batch, cursor-based append with no gaps, exhaustion and empty-batch stops, generation guard across a scope change, refresh window reload, per-view totals, reading-pane persistence and dual read-state sync, feed removal clearing the pane, and the absence of a new-arrival counter — and verify `npm test` passes all of them.
- [x] 9.4 Cover the scroll sentinel and footer in `src/__tests__/entryList.spec.ts` — observer root and lead-in, intersection-driven `loadMore`, silence while out of view or when exhausted, the post-load re-check in both geometries, progress text, and the three footer states — and verify `npm test` passes all of them.
