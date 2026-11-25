## Why

Every action in the application currently requires the mouse. `Escape` — closing a dialog or a row menu — is the only keyboard binding that exists, and the entry rows are not even focusable. For a reading workflow that is "open the app, pick a feed, work down the unread list", moving the hand off the keyboard between every article is the dominant cost.

A feed reader is also the kind of application where keyboard navigation pays off most: the actions are a small, stable vocabulary (next, open, read, star, refresh) repeated hundreds of times in a sitting. This change makes that vocabulary available as single keystrokes in a Vim-like style.

## What Changes

- Introduce a single keyboard cursor over the entry list. `j`/`k` move it and, matching what a mouse click already does, opening the row also marks it read. No new selection semantics are invented; the keyboard drives the existing one.
- Support Vim-style motions and counts: `gg`/`G` for the ends of the list, an optional numeric prefix (`5j`, `12G`), and `Ctrl-d`/`Ctrl-u` for half-page movement.
- Introduce a `g` prefix node so that `g a` jumps to *All entries* and `g s` jumps to *Starred*. The sidebar gets no cursor of its own; those two jumps are its entire keyboard surface.
- Bind the existing row, list, and reading-pane actions: `m` toggles read/unread, `s` toggles starring, `o` opens the original article in the browser, `v` switches the All/Unread tab, `A` runs the existing "mark this page read" action, `r` refreshes all feeds, and `J`/`K` scroll the reading pane.
- Add a `?` help overlay listing the key map, so the bindings are discoverable without reading source code.
- Coalesce burst input: holding `j` must not issue one content fetch, one read-state write, and one count refresh per repeat. Cursor movement stays local and instant, and the network work for a burst is flushed once when the burst settles.
- Keep the list still while the cursor moves: marking an entry read as the cursor passes it must not remove the row or re-sort the list under the cursor.

Explicitly **not** in this change: a sidebar cursor or any keyboard-driven feed/folder navigation beyond the two jumps, a focus model between panes, search, custom or user-configurable bindings, and numeric-prefix support on sidebar actions. No backend, storage, or IPC changes are required.

## Capabilities

### New Capabilities

- `keyboard-navigation`: how the application accepts keyboard input — the key map, motion and count handling, the `g` prefix, how keystrokes are gated while a dialog, menu, or text field has focus, cursor behaviour across reloads, drive-the-list paging, burst coalescing, and the help overlay.

### Modified Capabilities

<!-- None: every existing requirement keeps holding. Selection still marks an entry read, batches still refill an unread list, and batch loading still triggers on reaching the end of the loaded rows. The keyboard adds triggers and surfaces; it changes no existing guarantee. -->

## Impact

- `src/lib/` — a new key map module: a pure resolver from key sequence plus state to an action, so prefix, count, and gating rules are testable without a DOM.
- `src/App.vue` — owns the cursor, installs the single document-level key listener, and gains a burst flush path for read marking and content loading.
- `src/components/EntryList.vue` — renders the cursor row (the existing `.active` row style already covers it), scrolls the cursor into view using the known fixed row height, and asks for another batch when the cursor reaches the last loaded row.
- `src/components/ReadingPane.vue` — accepts `J`/`K` scrolling and resets its scroll position when a new entry is opened.
- `src/components/SettingsDialog.vue`, `src/components/FeedEditDialog.vue`, `src/components/RowMenu.vue` — keep their `Escape` handling but must stop being the place where keyboard state is decided; the container should suppress global bindings while they are open.
- `src/__tests__/` — new unit tests for the resolver and integration tests for cursor movement, burst coalescing, gating, and paging at the end of the list.
- No changes to Rust code, database schema, stored data, or the Tauri configuration.
