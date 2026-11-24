## Context

See proposal.md — Why. The relevant current state:

- `entries` stores full sanitized HTML (`content_html`, ammonia-cleaned at ingest) plus `title`, `link`, `published_at`, `created_at`, `is_read`, `is_starred`. It has no summary and no image.
- `db::list_entries` deliberately selects row metadata only (`id, feed_id, title, link, published_at, sort_key, is_read, is_starred`) and never `content_html`; the doc comment on `EntryListItemDto` states bodies are fetched on demand.
- Schema evolution already has a mechanism: `COLUMN_MIGRATIONS` in `db.rs` applies idempotent `ALTER TABLE ... ADD COLUMN` statements on every open, tolerating "duplicate column name".
- Parsing lives in `fetch::parse_feed`, which maps `feed_rs::model::Entry` into `ParsedEntry`; `feed-rs` 2.4 exposes `entry.summary`, `entry.media[].thumbnails[]/content[]`, `entry.links[]` (with `rel`/`media_type`), and `entry.base`.
- The frontend receives `feeds` in `App.vue`, and the backend already returns each feed's display title as `COALESCE(custom_title, title)`, so a feed label needs no new backend field.
- Taskbars: the app has no virtualization, no media cache, and no HTTP client outside `fetch::fetch_bytes`. `tauri.conf.json` sets `csp: null`, so remote images load in the webview without further configuration.

## Goals / Non-Goals

**Goals:**

- Every row detail is available without per-row network calls or extra queries.
- Extracting row detail is a pure, unit-testable function of a parsed feed entry (or of stored HTML, for the backfill).
- The list query stays single-table and index-friendly; adding columns must not change its plan.
- Existing databases gain detail on next start without re-fetching feeds.

**Non-Goals:**

- Imaging caching, offline covers, or any image bytes on disk.
- Virtualizing the list or changing batch size.
- Changing what the reading pane shows (it keeps the absolute timestamp).
- Deriving covers from the article page (`og:image`); that would require fetching the article, which the reader does not do.

## Decisions

### Store excerpt and cover as new `entries` columns

Add `excerpt TEXT` and `image_url TEXT` through the existing `COLUMN_MIGRATIONS` list, and extend `ParsedEntry` / `db::upsert_entry` to carry them. `list_entries` selects both and `EntryListItemDto` exposes them.

Alternatives:

- *Derive the excerpt inside the list query by stripping `content_html`* — rejected: it forces the list query to read 50 full bodies per page, undoing the reason `list_entries` skips content, and would make page latency proportional to article size.
- *Send `content_html` to the frontend and derive there* — rejected: directly contradicts `entry-list-pagination`'s exclusion of entry content, and ships large payloads for a two-line snippet.
- *A separate `entry_details` table* — rejected: two extra columns on a row already loaded do not justify a join.

### Excerpt is derived at parse time, plain text, 200 characters, cut on a word boundary

Precedence: `entry.summary` when it yields non-empty text, else the sanitized `content_html`. Both go through one helper that builds a `scraper::Html`, extracts text (entity decoding for free), collapses whitespace runs into single spaces, and truncates to ~200 characters at the last space, appending `…` only when truncation occurred. Text extraction rather than tag stripping keeps `<style>`/`<script>` bodies and attributes out by construction; `content_html` is already ammonia-cleaned.

Rationale for the length: two lines at typical list width is roughly 120-160 characters; 200 leaves slack for wider windows without making the payload meaningful (50 rows ≈ 10 KB of text). The truncation happens once at ingest, not per render.

### Cover URL precedence: media thumbnail → media content → image enclosure → first `<img>` in content

Order follows how explicitly a feed declares "this is the entry's image": `media:thumbnail` is literally a representative image; `media:content[type=image/*]` and `<link rel="enclosure" type="image/*">` are attachments; the first `<img>` in content is a heuristic and is tried last. Candidate URLs are rejected when they are not `http(s)` (skips `data:` payloads) or when resolution fails. Resolution uses `entry.base`, then the entry link, then the feed's `site_url`; `url::Url::join` handles relative and protocol-relative references.

Alternatives:

- *First `<img>` only* — rejected: it commonly picks a tracker pixel or an author avatar, and ignores the one signal a publisher actually curated.
- *`og:image` from the article page* — rejected: requires an article fetch per entry, plus a second HTTP dependency in ingest.

### Backfill runs once at schema init, in bounded batches, keyed on `excerpt IS NULL`

New inserts always write an excerpt (possibly the empty string), so `NULL` unambiguously means "predates the column". The backfill pages through `SELECT id, content_html FROM entries WHERE excerpt IS NULL AND id > ? ORDER BY id LIMIT 200`, deriving excerpt text and, from the first `<img>`, a cover URL (media metadata is gone for old rows, which is acceptable — a refresh upgrades them). It touches only `excerpt` and `image_url`; identity, read/starred state, content, and timestamps are never rewritten.

Alternative: a version-stamped `meta` table to record migration completion — rejected as ceremony for one idempotent, self-terminating predicate.

### Relative time is client-side, computed from `published_at ?? sort_key`, and ticks

A new module (`src/lib/time.ts`) exports `formatRelativeTime(iso, now)`; `EntryList.vue` holds a `now` ref advanced by a 60-second interval (cleared on unmount) and passes it in, so the formatter is pure and testable. Buckets: `<45s` → "just now", `<60m` → `Nm`, `<24h` → `Nh`, `<7d` → `Nd`, otherwise `toLocaleDateString()`. Negative deltas (clock skew, scheduled posts) clamp to "just now". The full timestamp goes on the element's `title` attribute — no tooltip component.

`sort_key` is used as the fallback rather than `created_at` because the list response does not carry `created_at`, and `sort_key` is already `COALESCE(published_at, created_at)` server-side.

### Feed label comes from data the frontend already holds

`App.vue` passes a `feedTitles: Record<number, string>` (or the existing `feeds` array) plus a `showFeedLabel` flag into `EntryList.vue`; the flag is false when `selection.kind === "feed"`. Unknown feed ids render no label rather than a placeholder.

### Uniform row height, image hidden on error, no layout shift

Rows become a fixed-height grid: an 84px cover column (only when `image_url` is present), a text column with a 2-line clamped title and 2-line clamped excerpt, and a meta line. The height is sized to the worst case, not chosen by eye: two title lines + two excerpt lines + the meta line + padding is about 115px, so the row is 116px. Undersizing it lets the meta line paint over the following row, which is why the row also clips (`overflow: hidden`) as a backstop for larger system fonts, with the meta line held at full size while the excerpt absorbs the shortfall. Fixed height is what makes lazy loading safe: an image arriving late cannot resize its row, so the scroll anchor never moves. `img` carries `loading="lazy"`, `alt=""` (decorative), and `@error` adds the entry id to a per-component `brokenImages` set that suppresses the element. Line clamping uses `-webkit-line-clamp` with `max-height` + `overflow: hidden` as the fallback.

## Risks / Trade-offs

- **Startup latency on large databases** → backfill is batched at 200 rows per statement, one-time, and self-terminating; if a 12k-entry database shows noticeable startup delay, the loop can be moved behind the first list load without changing the schema or specs.
- **Hotlinked covers leak reader IPs to publisher CDNs and make tracking pixels live requests** → accepted trade-off of the chosen approach; no mitigation beyond `loading="lazy"` (images are only requested for rows that come into view). Not silently "fixed" later without a proposal.
- **Some feeds' covers are 1×1 pixels, huge JPEGs, or aspect ratios far from the box** → fixed 72px box with `object-fit: cover` bounds layout damage; a minimum-dimension filter would require downloading bytes, which is out of scope.
- **Backfilled covers are weaker than fresh ones** (content `<img>` only, no media metadata) → an ordinary feed refresh upgrades them via the existing upsert path; the requirement covers this explicitly.
- **`-webkit-line-clamp` fallback** → if a future webview drops the property, `max-height` + `overflow: hidden` still bounds the row, at the cost of a hard cut instead of an ellipsis.
- **Excerpt computed from content can include boilerplate** ("Read more", share prompts) → accepted; the excerpt is a scanning aid, and the reading pane is authoritative.
- **`image_url` grows per row** → negligible next to `content_html`; no index is added, since the column is only ever read with the row.

## Migration Plan

1. Add the two columns via `COLUMN_MIGRATIONS` (idempotent, runs on open).
2. Run the batched backfill after `POST_MIGRATION` inside `init_schema`; rows processed before an interruption are already non-`NULL`, so a crash mid-backfill resumes where it stopped.
3. Rollback: the columns are additive and unread by older code paths, so an older build works against the migrated database unchanged; reverting the build leaves the columns in place.

## Open Questions

- Should the list row eventually show an author or category? Deferred; nothing in this design blocks adding another column later.
