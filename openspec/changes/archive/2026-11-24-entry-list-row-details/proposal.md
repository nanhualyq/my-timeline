## Why

The entry list renders a title, an absolute date, and a star — three pieces of information with no context. A reader scanning a timeline cannot tell which feed an entry came from, what it is about, or whether it carries a picture, so deciding what to open requires opening entries one by one. The list query already fetches entries that store full content and a feed id, so most of that context is cheap to surface; the missing pieces (a plain-text excerpt, a cover image URL) are naturally produced at ingest time and are simply not stored today.

## What Changes

- Each entry row SHALL show a cover thumbnail, the title, a short plain-text excerpt, the feed's display name, and a short relative time, with a full timestamp available on hover.
- Entries SHALL store a plain-text excerpt, derived at ingest from the feed's own summary when present and from the sanitized content otherwise.
- Entries SHALL store a resolved cover image URL, discovered at ingest from feed media, image enclosures, or the first image in the entry content, and resolved to an absolute URL.
- Existing stored entries SHALL be backfilled once with an excerpt and a cover URL, computed from the content already stored, so the list gains detail without requiring a re-fetch of every feed.
- The entry list response SHALL carry the new row metadata; it SHALL continue to exclude entry bodies.
- Cover images SHALL be referenced by their publisher URL and loaded lazily; the application SHALL NOT download, cache, or store image bytes.
- Rows SHALL keep a uniform height whether or not a cover exists, so lazy image loading cannot shift the reading position.

## Capabilities

### New Capabilities
- `entry-row-presentation`: what a single entry row displays and how each detail is derived and formatted — cover fallback, excerpt clamping, feed label visibility, relative time buckets and their absolute-time escape hatch.

### Modified Capabilities
- `entry-list-pagination`: the list-response exclusion of entry content is restated so that derived row metadata (excerpt, cover URL, feed label) is permitted while stored entry HTML remains excluded.
- `rss-reader`: ingest now persists a plain-text excerpt and a resolved cover image URL per entry, and existing entries are backfilled from stored content.

## Impact

- Database: two new `entries` columns (excerpt, cover URL) added through the existing column-migration list, plus a one-time backfill pass over entries whose excerpt is unset.
- Backend: `src-tauri/src/fetch.rs` (parse and extract), `src-tauri/src/db.rs` (columns, migration, backfill, list DTO and query), `src-tauri/src/lib.rs` (no command signature change expected).
- Frontend: `src/api.ts` (`EntryListItem`), `src/components/EntryList.vue` (row template and styles), `src/App.vue` (feed label lookup passed to the list).
- Tests: Rust tests for extraction priority, URL resolution, excerpt generation, and backfill; frontend tests for relative-time formatting and row rendering with and without a cover.
- Privacy/network: cover images are fetched by the webview directly from publisher hosts while the reader scrolls; offline covers are explicitly out of scope, as is any media cache or file deletion cascade.
