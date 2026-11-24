## Context

See proposal.md — Why. The backend already has the whole-library write: `mark_scope_read(EntryScope::All)` is one `UPDATE entries SET is_read = 1 WHERE is_read = 0`, and `db.rs` covers it with a test. The sidebar already has a `RowMenu` on every feed and folder row, `runAction()` + `emit("changed")` as the failure/reload path, and a per-row menu builder that hides the batch item at zero unread. The only missing pieces are a menu on the "All entries" row and the figure its label needs.

## Goals / Non-Goals

**Goals:**

- One entry point whose scope is unambiguous: the label states the library-wide unread count, not the current list's.
- No new command, no new query shape beyond one more column in an existing aggregate.

**Non-Goals:**

- A confirmation dialog or an undo affordance (see proposal.md).
- Any interaction between the global action and the current view: the action never narrows to what is on screen.

## Decisions

### The global action is the same mechanism, at the widest scope

```
   row            menu item                        write
   +------------+-------------------------------+--------------------------------+
   | feed       | Mark all as read (300)        | mark_scope_read(feed)          |
   | folder     | Mark all as read (1,204)      | mark_scope_read(folder)        |
   | All entries| Mark all as read (12,438)     | mark_scope_read(all)   <-- new |
   | Starred    | (no menu item)                | --                             |
   +------------+-------------------------------+--------------------------------+
```

The Starred row stays without the action. It is a shelf, not a queue, and the library-wide action already covers everything a star could point at.

Alternative considered: a header button in the "All entries" view. Rejected — the header acts on rendered rows everywhere else, so the same control would mean two different things in two places, and the sidebar is already where whole-range actions live.

### The unread total comes from the aggregate that already scans the table

`entry_counts` becomes `SELECT COUNT(*), SUM(is_starred), SUM(is_read = 0)`. One statement, one extra column, no extra round trip, and the sidebar's label cannot disagree with the badges it sits beside.

Alternative considered: call `count_entries("unread", all)` from the app. Rejected: it is a second query for a number the existing aggregate can return for free, and it would need its own refresh path.

### The menu appears only when it has something to say

`Sidebar.vue` already builds menu items per row and drops the batch item when the count is zero. The "All entries" row reuses that builder with the library-wide count, so a fully read library shows no menu at all rather than a disabled "Mark 0 read".

### Restoring after the global mark reuses the existing paths

The sidebar action calls the backend through `runAction()` and reports success with `emit("changed")`, exactly as the folder actions do. `App.reload()` then refreshes counts, reloads the window, and re-reads the open entry. Careful case: in an unread view the reload comes back empty, which the list already renders as "No entries." — no special handling.

## Risks / Trade-offs

- [One click can clear a library-wide backlog with no undo and no confirmation] → the label states the exact number, the action sits one level deeper than a feed's (inside a menu), and it lives only on the row that stands for the whole library.
- [A stale unread total could overstate the label] → the figure comes from the same aggregate that feeds the badges on every refresh, so it is no staler than the counts beside it; the backend returns the true number of rows changed and the tests assert label/effect agreement.
- [The "All entries" row gains a menu it never had, changing how the row looks] → the trigger uses the same `RowMenu` component and styling as the rows below it, so the row keeps its height and badge position.

## Migration Plan

No schema or data change. Rollback is dropping the menu from the "All entries" row and the extra count column; read state is unaffected.

## Open Questions

None.
