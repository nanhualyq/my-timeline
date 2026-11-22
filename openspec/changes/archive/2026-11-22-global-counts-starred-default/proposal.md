## Why

The two global sidebar rows — "All entries" and "★ Starred" — are the only sidebar items without a count badge, so users cannot see how many entries (or starred entries) exist without opening each view. In addition, selecting "★ Starred" inherits the previously active All/Unread tab, so a user who was reading the Unread list is dropped into a half-empty starred list and cannot tell that starred entries exist at all.

## What Changes

- **Sidebar counts on the global views**: "All entries" shows a badge with the total number of stored entries and "★ Starred" shows a badge with the total number of starred entries (both read and unread), using the same badge style as the existing feed/folder unread badges. Badges follow the existing convention of being hidden when the count is zero.
- **Starred view always shows all starred entries**: selecting "★ Starred" always lists every starred entry; the Unread tab is not offered while the Starred view is selected. The All/Unread filter continues to work normally on "All entries".
- **Counts stay fresh**: the two global counts are refreshed alongside the existing feed/folder unread counts — on load, on refresh, after read-state changes, and after starring/unstarring an entry.

## Capabilities

### New Capabilities

<!-- None: both behaviors extend existing capabilities. -->

### Modified Capabilities

- `entry-starring`: replaces the requirement "Starred view respects the unread filter" (the starred view now always shows all starred entries and offers no Unread tab) and adds a requirement that the sidebar Starred row shows the total starred count.
- `rss-reader`: adds a requirement that the sidebar "All entries" row shows the total stored-entry count.

## Impact

- **Backend (Rust)**: `src-tauri/src/db.rs` gains a small count query (total and starred entry counts in one statement) plus a test; `src-tauri/src/lib.rs` registers one new Tauri command. No schema change and no data migration.
- **Frontend (Vue)**: `src/api.ts` gains the count wrapper and type; `src/App.vue` holds the two counts, refreshes them with the existing `refreshCounts()`, and forces the All view for the starred selection; `src/components/Sidebar.vue` renders the two badges; `src/components/EntryList.vue` hides the Unread tab while the Starred view is selected.
- **Dependencies**: none added.
- **Compatibility**: additive except for the Starred view's filter behavior, which is a deliberate change to an existing `entry-starring` requirement.
