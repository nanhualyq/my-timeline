## Why

The entry list loads every matching entry in one unbounded query and renders
every row: `list_entries` has no `LIMIT`, it ships each entry's full
`content_html` body across the IPC bridge even though the list only displays a
title and a date, and the whole array is re-fetched on nearly every user
action. As subscriptions accumulate, both the query payload and the DOM grow
without bound, and the list cannot scale past the number of stored entries.

## What Changes

- The entry list loads in bounded batches of 50 and appends further batches as
  the reader scrolls, driven by a `(sort key, id)` cursor, so the list never
  loads every matching entry at once and consecutive batches never skip or
  repeat a row.
- A new `created_at` column on `entries` makes the sort key a total, immutable
  order: `COALESCE(published_at, created_at)`. A new expression index backs the
  global ordering.
- The list IPC response no longer carries entry bodies. `list_entries` returns
  a lightweight row type; the reading pane fetches a single full entry on
  demand through a new `get_entry` command.
- The reading pane stays open while further batches load instead of clearing
  when the selected entry scrolls out of the loaded rows.
- The list footer shows a browsing progress line ("150 / 12,438 entries
  viewed") backed by a new count command.
- **BREAKING** (IPC): `list_entries` gains `after` and `limit` parameters and
  returns a new `EntryListItem` shape; existing Rust call sites and TypeScript
  types must be updated together.
- Deferred to a dedicated change: notifying the reader about entries that
  arrived while they were scrolled away. A previous revision of this change
  tried to solve that with a banner; its definition of "new" was unsound (see
  design.md), and a continuous list has no page boundary hiding new rows in
  the first place.

## Capabilities

### New Capabilities

- `entry-list-pagination`: bounded, cursor-driven entry batches appended on
  scroll; stable ordering under refresh; progress display; on-demand entry
  body retrieval; reading pane persistence while the list loads.

### Modified Capabilities

- `entry-starring`: the starred-view requirements currently state that every
  starred entry is listed when the view is selected. Under pagination the full
  set is reachable through page navigation rather than rendered at once, so
  those requirements and their scenarios are reworded without changing the
  intent (global scope, no unread filter, read and unread alike).

## Impact

- `src-tauri/src/db.rs` — `created_at` column migration and backfill, new
  expression index, `list_entries` signature and result type, new
  `count_entries` and `get_entry` functions, `upsert_entry` must leave
  `created_at` untouched on conflict; a large existing test suite must be
  updated to the new signatures.
- `src-tauri/src/lib.rs` — registers the new commands.
- `src-tauri/src/fetch.rs` — existing tests call `list_entries` directly and
  must be updated.
- `src/api.ts` — `Entry` / `EntryListItem` split, `Cursor` type, changed or
  added functions.
- `src/App.vue` — append cursor instead of a page stack, separately loaded
  selected entry, split between "start over from the newest batch" and
  "reload the loaded window".
- `src/components/EntryList.vue` — progress footer, scroll sentinel and
  loading indicator.
- `openspec/specs/entry-starring/spec.md` — requirement rewording via delta.
- Test infrastructure (approved scope expansion): `vitest.config.ts`, a
  `test` script in `package.json`, and `src/__tests__/` — a fake Tauri backend
  mirroring the paging contract, App list-state tests, and scroll-sentinel
  tests. `vitest`, `@vue/test-utils`, and `jsdom` are new dev dependencies.
- No changes to sidebar counts, feed/folder management, OPML interchange, or
  settings.
