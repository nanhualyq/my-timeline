## Why

The application is currently an empty Tauri + Vue scaffold; its intended product is a cross-platform RSS reader. This change delivers the minimal viable reading loop: subscribe to feeds, fetch and parse them, store entries in SQLite, and read them with read/unread tracking.

## What Changes

- Replace the scaffold's example greet screen with a three-pane RSS reader UI (feed sidebar, entry list, reading pane).
- Add feed subscription management: add a feed by URL (with automatic feed discovery from a plain web page), and remove feeds.
- Add manual refresh of all feeds concurrently and manual refresh of a single feed; a failing feed records an error without blocking others.
- Add local persistence in SQLite: `feeds` and `entries` tables, deduplicated per feed by GUID (falling back to link).
- Add entry list filtering (all entries / unread only / per feed), reading pane rendering of entry HTML sanitized in the Rust backend, and read/unread toggling.
- Keep the product identity as `my-timeline` for now (renaming is deferred).

## Capabilities

### New Capabilities

- `rss-reader`: Subscription management, feed fetching/parsing/refresh, and entry browsing/reading with read/unread state.

### Modified Capabilities

- `desktop-shell`: The example `greet` command is replaced by real reader commands; the IPC bridge requirement's round-trip scenario must reference an actual reader command instead of the greeting example.

## Impact

- `src-tauri/Cargo.toml`: new dependencies (HTTP client, feed parser, SQLite, HTML sanitizer).
- `src-tauri/src/lib.rs`: new Tauri commands and the data layer; the example `greet` command is removed.
- `src/App.vue`: replaced by the reader UI; new Vue components for sidebar, list, and reading pane.
- `package.json`: no new frontend dependencies expected.
- Existing specs `desktop-shell` and `dev-build-workflow` continue to apply as-is.
