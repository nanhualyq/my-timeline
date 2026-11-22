## Why

The reader currently offers only a flat feed list with read/unread tracking: as subscriptions grow there is no way to group feeds, no way to bookmark important entries for later, and no way to migrate subscriptions to or from other readers. This change adds the three organizational capabilities users expect from a mature RSS reader — folders, starred entries, and OPML interchange.

## What Changes

- **Folders**: Feeds can be assigned to named, ordered folders (each feed belongs to at most one folder; no folder means "uncategorized"). The sidebar becomes a tree: global views first, then folders with their feeds nested underneath. Feeds can be moved between folders, folders can be created, renamed, reordered, and deleted; deleting a folder moves its feeds to uncategorized rather than removing them.
- **Starred entries**: Entries gain a persisted star flag, toggled from the entry list. The sidebar gains a global "Starred" view listing all starred entries. Starring is independent of read state and of the currently selected feed/folder.
- **OPML import**: The user can import an `.opml` file; outlined folders become folders and outlined feed URLs become feeds. Import is merge-only — URLs already subscribed are skipped, existing folder assignments are preserved. Import only inserts records; it does not trigger fetching.
- **OPML export**: The user can export all subscriptions, including folder grouping, as an `.opml` file.
- **Selection model**: Sidebar selection extends from `feed | none` to `all | starred | folder | feed` (a breaking change to the frontend selection type and the `list_entries` command contract).
- **New dependency**: `tauri-plugin-dialog` for choosing the `.opml` file on import/export.

## Capabilities

### New Capabilities

- `feed-folders`: Group subscriptions into named, ordered folders; browse entries by folder; manage folders (create, rename, reorder, delete-with-reassign).
- `entry-starring`: Star and unstar individual entries, and browse all starred entries in a global view.
- `opml-interchange`: Import subscriptions and folder structure from an `.opml` file with merge semantics, and export the current subscription set with folder grouping back to `.opml`.

### Modified Capabilities

<!-- None: existing rss-reader requirements remain satisfied as written.
     The browse/filter requirement already specifies "at least three views";
     folder and starred views are additive. -->

## Impact

- **Backend (Rust)**: `db.rs` schema additions (`folders` table, `feeds.folder_id`, `entries.is_starred`), new queries (folder CRUD, starred listing, folder-scoped entry listing), new Tauri commands; `lib.rs` command registration; new OPML parse/generate module.
- **Frontend (Vue)**: `src/api.ts` command wrappers and types; `Sidebar.vue` tree rendering and folder actions; `App.vue` selection state becomes a discriminated union; `EntryList.vue` star toggle and Starred view.
- **Dependencies**: adds `tauri-plugin-dialog` (Rust + JS); OPML parsing done in Rust (no new JS XML dependency).
- **Data**: existing databases migrate in place — `folder_id` defaults to NULL (uncategorized), `is_starred` defaults to 0; no user data is lost.
