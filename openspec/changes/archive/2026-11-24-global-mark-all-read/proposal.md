## Why

Batch marking now exists for a feed, a folder, and whatever the list has loaded, but there is no way to say "I am done with everything in the library". A reader who has fallen a long way behind has to clear subscriptions one at a time, and the "All entries" row — the one row that stands for the whole library — is the only sidebar row with no menu at all.

## What Changes

- Give the "All entries" sidebar row a row menu offering "Mark all as read (N)", where N is every unread entry in the library, regardless of the current view or selection.
- Report the library-wide unread count from the backend so that label can state its size without a second query.
- The "Starred" row keeps no batch action: the global action already covers everything it holds, and Starred is a shelf rather than a queue.
- Not included, deliberately: a confirmation dialog, an undo affordance, and any batch action that marks entries unread.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `bulk-mark-read`: the whole-collection requirement stated that the "All entries" row offers no batch action; it now does, and the scenario asserting its absence is replaced.

## Impact

- `src-tauri/src/db.rs`: `entry_counts` also returns the library-wide unread total.
- `src/components/Sidebar.vue`: the "All entries" row gains a menu whose only item is the global batch read.
- `src/App.vue`: passes the unread total down and restores the list through the existing reload path.
- `src/api.ts`: `EntryCounts` gains the new field.
- `src/__tests__/`: fake backend mirrors the extra count; sidebar and app specs cover the global action.
