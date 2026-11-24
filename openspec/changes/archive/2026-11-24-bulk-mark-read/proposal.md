## Why

Read state can only change one entry at a time, and only by opening it. A reader who has scanned a feed, a folder, or the global unread queue has no way to say so: they must open every entry or leave the unread badges permanently wrong. The reader needs batch marking that never touches entries the user has not been shown, plus one explicit whole-subscription escape hatch.

## What Changes

- Add a **Mark read** action to the entry list header that marks the entries already loaded in the list — and only those — as read.
- Add a per-row **Mark read above** action that marks the loaded entries above that row as read, excluding the row itself.
- Add a **Mark all as read** item to a feed's and a folder's sidebar row menu that marks the whole subscription or folder read, regardless of what the list currently shows.
- Reload the list after every batch mark so the visible page refills instead of leaving a gap the reader has to scroll through.
- Not included, deliberately: multi-select in the list, an undo affordance, a confirmation dialog, a global "mark every stored entry read" action, and batch marking back to unread.

## Capabilities

### New Capabilities

- `bulk-mark-read`: Batch read marking — which scopes each action covers, where each action is offered, and how the list and unread counts respond.

### Modified Capabilities

None. Single-entry read toggling (`rss-reader`) is unchanged, and the existing list, starring, and folder capabilities keep their requirements.

## Impact

- `src-tauri/src/db.rs`: one bulk update over an explicit id list and one scoped update, with tests.
- `src-tauri/src/lib.rs`: two new Tauri commands and their registration.
- `src/api.ts`: two new wrappers.
- `src/components/EntryList.vue`: header action, per-row action, and labels carrying the affected count.
- `src/components/Sidebar.vue`: a per-row menu item built from each feed's and folder's unread count.
- `src/App.vue`: batch handlers plus the reload rule that depends on the active view.
- `src/__tests__/`: fake backend commands and specs covering the three scopes.
