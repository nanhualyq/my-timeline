## Context

See proposal.md — Why. The constraints that shape this design are already in the code:

- `db.rs` has `filter_conditions(view, scope)`, which produces the WHERE clause for a scope (`all` / `feed` / `folder` / `starred`) combined with a view (`all` / `unread`). Listing and counting share it, so a scoped bulk update can reuse it and cannot disagree with what the list shows.
- The list is paginated: the first batch starts at the newest entry and later batches append downwards. An entry already loaded therefore always has every newer entry of the same list loaded too.
- `App.vue` already distinguishes two reload paths: `startOver()` (clears rows, refetches batch one, bumps a generation counter that discards in-flight responses) and `fetchWindow(depth)` (refetches the loaded window in place).
- `Sidebar.vue` already gives every feed row and every folder row a `RowMenu`, and `runAction()` + `emit("changed")` is how a sidebar action surfaces errors and triggers a reload.
- There are no confirmation dialogs for reversible data actions except feed removal, and no undo infrastructure or list multi-select anywhere in the application.

## Goals / Non-Goals

**Goals:**

- Three concrete scopes, each with an implementation that matches its reach: rendered rows, rendered rows above a row, whole subscription.
- No action can mark an entry the reader has not been shown, except the one action whose label states the full count.
- The list keeps its shape: a page never ends up half-empty, and a reader who scrolled deep is not thrown back to the top.

**Non-Goals:**

- List multi-select, keyboard-driven batch actions, and an undo affordance (see proposal.md).
- Changing how single entries are marked read on open.
- Any new schema, migration, or index.

## Decisions

### Three scopes, two implementations

```
   scope                                  source of ids          write path
   +------------------------------------+---------------------+------------------------+
   | C  row action: rendered, above row | entries[0..idx]     | mark_read_many(ids)    |
   | A  header action: rendered rows    | entries[]           | mark_read_many(ids)    |
   | S  sidebar row: whole subscription | server-side scope   | mark_scope_read(scope) |
   +------------------------------------+---------------------+------------------------+
```

The header and row actions are client-side selections over the rows already in memory; only the sidebar action reaches entries the list has never loaded. This is why the row action needs no ordering mathematics: because batches append downwards from the newest entry, `entries.slice(0, idx)` is exactly "everything above this row", unread filtered locally.

Alternative considered: a server-side boundary command taking a `(sort_key, id)` cursor. Rejected — it duplicates the paging cursor predicate (`sort_key > ? OR (sort_key = ? AND id > ?)`) for an answer the client already has, and it would let the action reach unloaded entries.

Alternative considered: making all three actions scope-wide. Rejected in the spec: it makes a single click able to clear an entire backlog the reader never saw.

### Two commands, named so the disallowed operation cannot be expressed

```rust
mark_read_many(ids: Vec<i64>) -> Result<i64, String>      // fixed to read = true
mark_scope_read(scope: EntryScope) -> Result<i64, String> // fixed to read = true
```

Neither takes an `is_read` flag. The spec forbids batch marking unread, so the API should not have a way to ask for it — a future "batch unread" must be a deliberate new command, not a boolean the UI happens to pass.

`mark_scope_read` reuses `filter_conditions("unread", scope)` and issues one `UPDATE entries SET is_read = 1 WHERE <conditions>`. Both commands run inside a transaction and return the number of rows changed, which the caller ignores for display purposes but tests assert.

Alternative considered: one generic `set_read(ids_or_scope, value)`. Rejected as above, and it would make two very different reach profiles look like one operation.

### Row action excludes its own row

The affordance sits on the row, so its literal reading is "everything above this is done". Excluding the row also means the entry the reader is pointing at is never silently marked read by a hover click.

### Availability follows whether the list is a triage queue

Both in-list actions appear for a feed, a folder, and the unread view of "All entries" — lists whose contents are things still to be dealt with. They are absent from "All entries" with the All tab (an archive that also holds read entries) and from Starred (a shelf). The sidebar action is not governed by the list at all: it is attached to a subscription row and works while any view is showing, including Starred.

### Restoring the list depends on the active view

```
   view = unread  ->  startOver()          marked rows vanish, batch one refills the page
   view = all     ->  patch marked ids     rows keep their positions and dates, dots disappear
```

`startOver()` in the all view would scroll the reader back to the top, which is a bug, not a refresh. Patching is exact because the client knows the ids it just marked, so no refetch is needed for the all view.

The sidebar action is different: it can mark entries that are not loaded, so the client cannot patch by ids. It reports through the existing `emit("changed")` path and reloads — `startOver()` in the unread view, `fetchWindow(depth)` in the all view — with `selectedEntry` patched when the open entry belongs to the marked scope.

### Counts come from data already in memory or already in the DTOs

- Header action: `select(e => !e.is_read).length` over `entries`.
- Row action: the same filter over `entries.slice(0, idx)`.
- Sidebar action: `feed.unread_count` / `folder.unread_count`, already returned by `list_feeds` / `list_folders`.

No new query, and every label states a number that equals what the action does.

### Menu items become per-row, not module constants

`Sidebar.vue` currently holds module-level `feedMenuItems` / `folderMenuItems`. Because the sidebar batch item must be hidden for a feed or folder with no unread entries, the item list becomes a function of the row. The row action in the list is likewise rendered only when something above is unread, and it floats over the row's top-right corner on hover instead of taking a flex column: a control the reader uses occasionally must not permanently narrow every row. The header action stays rendered but is disabled when the rendered page holds no unread entries, so its label can never read as zero and the header layout does not shift as rows are marked.

## Risks / Trade-offs

- [A batch of 1,200 ids in one statement can approach SQLite's bound-parameter limit] → chunk ids into groups of 500 inside a single transaction; assert the chunk path in a test with more ids than one chunk.
- [The row action is hover-revealed, so touch users cannot reach it] → rows are not keyboard-reachable today either, so this does not regress existing navigation; it is revealed on `:hover` and `:focus-visible`, and overlapping the row keeps it from costing a column.
- [No undo and no confirmation means the sidebar action can silently clear a large backlog] → the menu label states the unread count, and the action exists only on a single feed or folder, never globally.
- [A batch mark while a refresh is in flight can produce a surprising row set] → reuse the existing `generation` counter so reloads triggered by the mark discard stale in-flight responses, the same way scope changes already do.
- [Counts can go stale between render and click when a background refresh adds entries] → the label is a best-effort figure from the loaded page; the backend returns the true number changed, and tests assert label/effect agreement only in the absence of concurrent refresh.

## Migration Plan

No schema, data, or dependency changes: read state already exists per entry. Rollback is removing the two commands, the three affordances, and their tests — stored read state is unaffected.

## Open Questions

- Whether the header action should also get a keyboard shortcut. Deferrable: it changes no scenario in the spec and adds no backend surface.
- Whether a feed's sidebar action should also be offered for a folder while that folder is collapsed. Deferrable; the row menu exists in both states, so the current answer is yes.
