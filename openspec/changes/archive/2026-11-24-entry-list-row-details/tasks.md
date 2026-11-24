## 1. Ingest: derive excerpt and cover URL

- [x] 1.1 Add `excerpt: String` and `image_url: Option<String>` to `ParsedEntry` in `src-tauri/src/fetch.rs`, and verify the crate still compiles (`cargo check` in `src-tauri/`)
- [x] 1.2 Implement a plain-text helper (HTML → text via `scraper`, whitespace collapsed, truncated at ~200 chars on a word boundary with `…` only when cut) and verify unit tests cover markup stripping, whitespace collapsing, word-boundary truncation, and text shorter than the limit
- [x] 1.3 Implement cover extraction with the precedence media thumbnail → image media content → image enclosure link → first `<img>` in sanitized content, resolving candidates with `entry.base`, then the entry link, then the feed `site_url`, and verify unit tests cover each precedence step, relative and protocol-relative resolution, and rejection of non-`http(s)` URLs
- [x] 1.4 Wire both helpers into `parse_feed` so the excerpt prefers `entry.summary` and falls back to content, and verify tests cover summary-preferred, content-only, and no-image cases

## 2. Storage: columns, upsert, list query, backfill

- [x] 2.1 Add `entries.excerpt` and `entries.image_url` to `COLUMN_MIGRATIONS` in `src-tauri/src/db.rs` and verify a schema test asserts both columns exist after `init_schema`
- [x] 2.2 Extend `upsert_entry` to persist excerpt and cover URL on insert and on conflict update, and verify a test shows a re-upsert of the same GUID updates both fields without creating a second row
- [x] 2.3 Add `excerpt` and `image_url` to `EntryListItemDto` and to the `list_entries` select and row mapper, and verify the pagination/ordering tests still pass and a test asserts both values are returned while `content_html` is not
- [x] 2.4 Implement the batched backfill (`excerpt IS NULL`, 200 rows per statement) deriving excerpt and content-image cover URL, and verify tests show pre-existing rows gain both fields, read/starred/content/published state is untouched, and a second run processes zero rows
- [x] 2.5 Call the backfill from `init_schema` after `POST_MIGRATION` and verify `cargo test` passes for the database module

## 3. Frontend: list item shape and relative time

- [x] 3.1 Add `excerpt: string` and `image_url: string | null` to `EntryListItem` in `src/api.ts` and verify `npm run check` passes
- [x] 3.2 Create `src/lib/time.ts` exporting a pure `formatRelativeTime(iso, now)` with the spec's buckets, future-delta clamping, and absolute-date fallback, and verify unit tests cover each bucket boundary and a future timestamp
- [x] 3.3 Drive the labels from a `now` ref refreshed by a 60-second interval in `EntryList.vue`, cleared on unmount, and verify a test advances the clock and observes a changed label without a reload

## 4. Frontend: row presentation

- [x] 4.1 Extend `EntryList.vue` with feed-title lookup data and a `showFeedLabel` flag, and verify tests show the label in the all-entries view and its absence in a single-feed view
- [x] 4.2 Render the fixed-height row: 72px lazy cover, 2-line clamped title, 2-line clamped excerpt, meta line of feed label and relative time with the full timestamp in `title`, keeping the unread indicator and star control, and verify tests assert all rendered pieces and the uniform height class/structure with and without a cover and without an excerpt
- [x] 4.3 Hide a cover whose image fails to load without leaving a placeholder, and verify a test dispatches the image error event and asserts the slot disappears with the row structure unchanged
- [x] 4.4 Pass feed titles and the label flag from `src/App.vue` and verify `npm run check` passes and the list renders a feed label for an entry from a renamed feed
- [x] 4.5 Update existing `EntryListItem` fixtures in `src/__tests__/` and verify `npm test` passes

## 5. Integration check

- [x] 5.1 Run `cargo test`, `npm test`, and `npm run check` and verify all pass
- [x] 5.2 Manually start the app against an existing database and verify rows show covers where available, excerpts, feed labels, relative times, and that scrolling does not shift rows as images arrive
