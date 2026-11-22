## Context

The repo is a fresh Tauri 2 + Vue 3 scaffold (example `greet` command, single `App.vue`). Scope for this change is the L0 reading loop defined in proposal.md: all business logic lives on the Rust side (confirmed route), identity stays `my-timeline` (rename deferred).

## Goals / Non-Goals

**Goals:**

- A working subscribe → fetch → read loop persisted in a local SQLite file.
- A data layer simple enough that L1 (folders, star, FTS5, OPML) can be added without schema rewrites where cheap.

**Non-Goals:**

- Auto/background refresh, folders, starring, search, OPML, rules, full-text fetching, sync (L1/L2).
- Multi-window or mobile targets; "cross-platform" means desktop (Windows/macOS/Linux) via Tauri.

## Decisions

**D1. All logic in Rust (vs. tauri-plugin-http / tauri-plugin-sql in the frontend).**
RSS/Atom parsing (`feed-rs`), HTTP with encoding/redirect/timeout handling (`reqwest`), and SQLite (`rusqlite`) are mature on the Rust side; the fetch loop must outlive the webview for future auto-refresh; business logic stays unit-testable without a webview. Alternative rejected: plugin-based frontend route — faster for a spike, but parsing ecosystem in TS is weak and logic would be migrated back later.

**D2. SQLite schema: two tables, `feeds` and `entries`.**

```
feeds                          entries
-----                          -------
id INTEGER PK                  id INTEGER PK
url TEXT UNIQUE                feed_id INTEGER FK -> feeds.id ON DELETE CASCADE
title TEXT                     guid TEXT
site_url TEXT                  title TEXT
last_fetched_at TEXT           link TEXT
last_error TEXT                content_html TEXT
                               published_at TEXT
                               is_read INTEGER DEFAULT 0
                               UNIQUE(feed_id, guid)
```

Read state lives on the entry (simplest for L0). Dedup enforced by the `UNIQUE(feed_id, guid)` constraint; `guid` is backfilled with the entry link when the feed omits it (falls back to link per spec). Deletion cascades so removing a feed removes its entries. No folder/star columns yet — adding nullable columns later is a trivial migration; premature `folder_id` would invite L1 complexity now.

**D3. Refresh architecture: one shared fetch-and-store routine.**
`refresh_feed(feed_id)` does fetch → parse → upsert → update `last_error`/`last_fetched_at` for one feed. `refresh_all()` runs it concurrently over all feeds (bounded concurrency, per-request timeout) and collects per-feed results. A single failure sets that feed's `last_error` and never aborts the batch. Both are plain Tauri commands; the frontend re-polls list state after the command resolves (L0 data volumes make full re-pull simpler than incremental updates).

**D4. Feed discovery on add.**
`add_feed(url)`: try fetching the URL and parsing it as a feed; if parsing fails and the response is HTML, scan for `<link rel="alternate" type="application/rss+xml|atom+xml">` and retry with that href (resolved against the base URL). Reject with a typed error the frontend can display.

**D5. HTML sanitization in Rust with `ammonia`, before data reaches the webview.**
Stored `content_html` is sanitized at render time (or at ingest — ingest-time keeps untrusted bytes out of queries; render-time allows re-sanitizing if the allowlist changes; decision: sanitize at ingest so the DB only ever holds safe HTML). The frontend renders with `v-html` only against this sanitized output. `ammonia` allows a conservative tag/attribute allowlist (p, a, img, h1–h6, ul/ol, li, blockquote, code, pre, br, em, strong; `href`/`src` limited to http/https).

**D6. IPC surface (7 commands).**
`add_feed(url)`, `remove_feed(id)`, `refresh_all()`, `refresh_feed(id)`, `list_feeds()`, `list_entries(view)` where `view` is all | unread | feed(id); plus `set_read(entry_id, bool)`. All return serde-serialized DTOs; errors surface as `Err(String)`-style messages the UI can show.

**D7. Frontend structure.**
Single window; `App.vue` becomes a three-pane layout backed by composables that call `invoke` and re-pull after each mutation. No state library — `ref`s and explicit refresh are enough at this scale.

## Risks / Trade-offs

- [Feeds with bad encodings (GBK etc.) or pathological HTML] → `reqwest` + `feed-rs` handle common encodings; failures land in `last_error`, never crash the app.
- [`guid` collisions across differently-shaped feeds] → constraint is per-feed (`feed_id, guid`), so cross-feed collisions are impossible by construction.
- [Sanitize-at-ingest freezes the allowlist] → acceptable for L0; re-sanitizing existing rows is a one-off migration if the allowlist changes later.
- [Full UI re-pull after every action] → fine at L0 scale; if thousands of entries make it sluggish, switch to targeted invalidation without changing the specs.
- [`reqwest`/`feed-rs`/`rusqlite` compile times lengthen first build] → one-time cost; `rusqlite` bundled feature avoids system SQLite dependency differences across platforms.

## Migration Plan

No existing user data. The scaffold's `greet` command and example UI are replaced; the `desktop-shell` IPC requirement is updated in this change's delta spec accordingly.

## Open Questions

None blocking. Deferred by scope: exact placement of DB file (app data dir via `dirs`/Tauri path API) and window sizing for a three-pane layout — implementation details that do not affect the specs.
