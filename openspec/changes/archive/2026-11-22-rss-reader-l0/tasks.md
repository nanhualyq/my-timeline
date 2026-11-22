## 1. Data layer (Rust)

- [x] 1.1 Add dependencies (`reqwest`, `feed-rs`, `rusqlite` with bundled feature, `ammonia`, `url`, `tokio` concurrency primitives) and verify `cargo check` passes in `src-tauri`
- [x] 1.2 Create the SQLite schema (`feeds`, `entries` with `UNIQUE(feed_id, guid)` and `ON DELETE CASCADE`) at app startup in the app data dir and verify the file is created with both tables
- [x] 1.3 Implement feed/entry repository functions (insert/update feed, upsert entry with guid-fallback-to-link dedup, list feeds with unread counts, list entries by view, set read state, delete feed cascading) and verify with Rust unit tests covering the dedup and cascade cases

## 2. Fetch, parse, and refresh (Rust)

- [x] 2.1 Implement the shared fetch-and-store routine (fetch with timeout, parse with feed-rs, sanitize content HTML with ammonia at ingest, upsert, update `last_fetched_at`/`last_error`) and verify with a unit test against a fixture feed
- [x] 2.2 Implement `refresh_all` (bounded-concurrent invocation of the routine; per-feed failure recorded without aborting the batch) and verify a test where one feed fails and others still store entries
- [x] 2.3 Implement `add_feed` with page-to-feed discovery (`<link rel="alternate">` fallback) and typed error output and verify tests for direct feed, discoverable page, and invalid URL cases

## 3. Tauri command surface

- [x] 3.1 Register the 7 commands (`add_feed`, `remove_feed`, `refresh_all`, `refresh_feed`, `list_feeds`, `list_entries`, `set_read`) replacing the example `greet` command and verify `cargo check` passes
- [x] 3.2 Verify a round-trip `invoke("list_feeds")` from the frontend returns data without console errors (satisfies the modified `desktop-shell` IPC requirement)

## 4. Frontend reader UI

- [x] 4.1 Build the three-pane layout (feed sidebar, entry list, reading pane) in `App.vue`/components and verify `npm run check` passes and the layout renders on launch
- [x] 4.2 Implement sidebar behavior: add-feed input (with error display), per-feed remove and refresh buttons, unread counts, error indicator on failing feeds, and verify manually by adding a good feed, a bad URL, and a failing feed
- [x] 4.3 Implement entry list: all/unread/per-feed views, newest-first ordering, refresh-all action, select-to-open, and verify `npm run check` plus manual switching between views
- [x] 4.4 Implement reading pane: sanitized HTML rendering via `v-html`, read/unread toggle reflected in counts and surviving restart, and verify manually that a feed entry containing a `<script>` tag does not execute
- [x] 4.5 Run the full manual acceptance pass (add feed → refresh all → refresh single → read/unread → remove feed → restart persistence) and verify each spec scenario in `specs/rss-reader/spec.md` holds

## 5. Validation

- [x] 5.1 Run `openspec validate rss-reader-l0 --strict` and `npm run check` and verify both exit 0
