## Context

Folder ordering is currently user-controlled end to end: `db.rs` stores a `position` column (set by `create_folder` via `MAX(position)+1` and rewritten by `db::reorder_folders`), `list_folders` sorts with `ORDER BY f.position, f.id`, `lib.rs` exposes a `reorder_folders` Tauri command, `src/api.ts` wraps it as `reorderFolders`, and `Sidebar.vue` renders "Move folder up/down" buttons that call it. `FolderDto` and the TS `Folder` interface both carry `position`. The reordering requirement lives in `openspec/specs/feed-folders/spec.md`. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**

- Folders always appear in default string comparison order of their names.
- Remove the reordering feature completely: UI controls, client API, Tauri command, DB function, and the `position` column from the schema and DTOs.

**Non-Goals:**

- Migrating existing databases to drop their `position` column (explicitly out of scope per user decision).
- Changing how feeds are ordered within a folder (`ORDER BY title COLLATE NOCASE` stays as is).
- Enforcing unique folder names.

## Decisions

1. **Sort in SQL, not in the frontend.** `list_folders` becomes `ORDER BY f.name, f.id`. SQLite's default (BINARY) collation on TEXT is exactly the default string comparison requested, and keeping ordering in the single query that already produces the folder list avoids duplicating ordering logic in `Sidebar.vue`. Alternatives considered: sorting in JS with `Array.prototype.sort()` (default UTF-16 code-unit order, which can diverge from the DB's UTF-8 byte order for supplementary characters) and `localeCompare` (locale-dependent, so not "default" comparison). The `f.id` tie-break keeps the order deterministic when two folders share a name.
2. **Drop `position` from the schema definition rather than migrate.** `FOLDERS_TABLE` loses the `position` column; `create_folder` inserts only `name`; `FolderDto` and the TS `Folder` interface lose the field. No `DROP COLUMN` migration is added, per the decision to not handle migrations. See Risks for the consequence on already-created databases.
3. **Remove the feature end to end in one change.** Delete `db::reorder_folders`, the `reorder_folders` command handler and its registration in `lib.rs`, `reorderFolders` in `api.ts`, and `moveFolder` plus the two buttons in `Sidebar.vue`, rather than leaving dead code behind.

## Risks / Trade-offs

- [Databases created before this change have `position INTEGER NOT NULL` with no default, so an `INSERT` that omits `position` fails with a NOT NULL constraint error.] → Accepted per the explicit decision to skip migration handling; at this development stage the remedy is deleting the local database file. Noted here so the behavior is not a surprise.
- [Case-sensitive ordering puts "Banana" before "apple", which may look odd to users expecting alphabetical, case-insensitive order.] → This is the requested default string comparison; pinned by a scenario in the delta spec so the behavior is intentional and testable.
- [`CREATE TABLE IF NOT EXISTS` will not alter existing tables, so the stale `position` column remains in old databases.] → Harmless: no code reads or writes it after this change.

## Migration Plan

None. Rollback is a plain git revert of the implementation commit.

## Open Questions

None.
