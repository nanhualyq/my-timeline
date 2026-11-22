## Context

The app is a Tauri 2 + Vue 3 + SQLite reader. The backend exposes flat Tauri commands (`list_feeds`, `list_entries`, ...) over a single `Connection` behind a `Mutex` (`src-tauri/src/db.rs`, `lib.rs`); the frontend holds a single `selectedFeedId: number | null` in `App.vue` that drives `list_entries(view, feed_id)`. `Sidebar.vue` renders a flat feed list. There is no schema migration mechanism — `db::open` runs `CREATE TABLE IF NOT EXISTS` idempotently, so additive schema changes fit the existing pattern. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**

- Add folders (single-folder-per-feed, flat, ordered), entry starring, and OPML import/export with the behaviors specified in `specs/`.
- Keep the existing command/DTO pattern; extend rather than restructure.
- Migrate existing databases in place with no data loss.

**Non-Goals:**

- Nested sub-folders beyond what OPML import produces (import may create nesting, but the UI only needs to render what the data model holds — see Open Questions).
- Auto-fetching on import; scheduled/background refresh.
- Sync, feed discovery changes, or read-state/OPML-embedded `outline` read flags (starred/read state is NOT exported to OPML).

## Decisions

### D1: Schema — additive columns + one new table

```sql
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    position INTEGER NOT NULL
);
ALTER TABLE feeds ADD COLUMN folder_id INTEGER REFERENCES folders(id);
ALTER TABLE entries ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_entries_starred ON entries(is_starred, published_at DESC);
```

- `ALTER TABLE ... ADD COLUMN` is guarded by running it tolerantly (ignore "duplicate column" errors) or by checking `PRAGMA table_info`, since `init_schema` runs on every open. Chosen: attempt the ALTER and ignore the duplicate-column error — no version-number migration framework needed for one additive change.
- `folder_id` nullable = uncategorized (user decision: delete-folder reassigns to uncategorized = NULL).
- Alternative considered: a `feed_folders` join table — rejected, user chose single-folder membership.

### D2: `list_entries` selection becomes a scope parameter

Extend the existing command instead of adding parallel commands:

```
list_entries(view: "all"|"unread", scope:
    {kind:"all"} | {kind:"feed", id} | {kind:"folder", id} | {kind:"starred"})
```

- The frontend `selection` discriminated union (from App.vue) maps 1:1 onto `scope`. `starred` scope filters `is_starred = 1`; `folder` scope joins via `feeds.folder_id`. The existing `feed_id: Option<i64>` argument is replaced (**breaking** to the command contract, confined to `src/api.ts` + `App.vue` + `Sidebar.vue`).
- Alternative considered: separate `list_starred_entries` / `list_folder_entries` commands — rejected; one query site keeps ordering/filter logic in one place, matching the existing `match (view, feed_id)` structure.
- Starred view is global (user decision): `{kind:"starred"}` never combines with feed/folder scope; the union type makes that unrepresentable.

### D3: Folder ordering via `position`

- `position` assigned as `max(position)+1` on create; reorder updates the affected rows' positions in one transaction.
- Alternative considered: `sort_key` float or name-only ordering — rejected; integer positions are sufficient for a flat, manually ordered list.

### D4: Unread counts

- `FeedDto.unread_count` unchanged; add `unread_count` to `FolderDto` via the same correlated-subquery pattern already used for feeds (`SUM` over the folder's feeds). Sidebar aggregates are computed in SQL, not in the frontend.

### D5: OPML handled entirely in Rust

- New module `src-tauri/src/opml.rs`: parse with a small hand-rolled or `roxmltree`-based reader (prefer `roxmltree`: pure-Rust, no network, small); export by generating XML with escaped attributes.
- File picking via `tauri-plugin-dialog` (`rfd` under the hood): `open()` returns a path, Rust reads the file bytes (no `tauri-plugin-fs` needed — the backend reads the path directly, sidestepping webview fs permissions). Export: `save()` returns a path, Rust writes it.
- Import algorithm: walk outlines depth-first; an outline with `xmlUrl` is a feed assigned to the folder of its **top-level ancestor** outline (nested folders are flattened); an outline without `xmlUrl` that contains feed outlines creates a folder. Duplicate URL → skip (reuse `feed_id_by_url`), preserving existing `folder_id`. All inserts inside one transaction. No fetch afterwards (user decision).
- Import/export result DTO: `{added, skipped, folders_created}` / `{feeds_exported}` for a visible summary.

### D6: Sidebar tree rendering

- `list_folders` returns folders (by `position`) each with nested `feeds: FeedDto[]`, OR the frontend groups `list_feeds()` results by `folder_id`. Chosen: **backend returns nested structure** (`FolderDto { ..., feeds }`) so unread aggregation and ordering live in one place; `list_feeds` stays for the uncategorized group or is folded into the same command returning `{folders, uncategorized, starred_count}`.
- Folder actions (create/rename/delete/reorder, move feed) are individual commands: `create_folder`, `rename_folder`, `delete_folder`, `reorder_folders`, `move_feed_to_folder`.

## Risks / Trade-offs

- [Duplicate-column ALTER fails on every subsequent open] → run ALTER once, swallow only the "duplicate column name" error (SQLite error code 1, checked by message), keep other errors fatal.
- [OPML files in the wild are messy (bad encoding, `about:` URLs, missing titles)] → validate and skip malformed outlines, count them as skipped; reject only non-well-formed XML with a visible error (spec'd).
- [Breaking `list_entries` contract] → single frontend call site (`src/api.ts`); `vue-tsc` (`npm run check`) catches drift.
- [`roxmltree` adds a Rust dependency] → it is pure-Rust and dev-visible only at compile time; alternative (hand-rolled parser) rejected as error-prone against real-world OPML.

## Migration Plan

1. Ship schema change; existing DBs gain `folder_id = NULL`, `is_starred = 0` — all feeds start in uncategorized, matching the sidebar's existing flat look plus an "Uncategorized" group.
2. No rollback path needed beyond reverting code; the additive columns are ignored by older code.

## Open Questions

None — nested OPML folders are flattened to their top-level ancestor folder at import (folders are flat per the chosen model; see spec scenario "Importing an OPML file with nested folders").
