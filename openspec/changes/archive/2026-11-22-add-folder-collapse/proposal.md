## Why

As the feed list grows, every folder permanently shows all of its nested feeds, making the sidebar long and hard to scan. Users need a way to fold a folder's feeds away while keeping the folder's name and aggregated unread count visible.

## What Changes

- Folder rows gain a leading expand/collapse icon button (placed before the folder name) that toggles the visibility of the folder's nested feed rows.
- All folders start collapsed on every app launch; expansion state is not persisted and resets on restart.
- Selecting a folder (clicking its name) keeps browsing that folder's entries and does NOT change expansion state — only the icon button toggles it.
- The folder row now shows the toggle icon, the folder name, its aggregated unread count, and the existing menu control (amends the "row shows only name and count" rule).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `feed-folders`: Folder rows become collapsible — a leading toggle icon controls whether nested feeds are shown, folders default to collapsed each launch, and the folder row's allowed contents gain the toggle control alongside name, count, and menu.

## Impact

- `src/components/Sidebar.vue` — folder row markup, expansion state (per-session `ref`, default collapsed), conditional rendering of nested feed rows, toggle styling, and the linearized `items` computation.
- `openspec/specs/feed-folders/spec.md` — amended via delta spec ("Sidebar presents folders as a tree" and "Folder row actions live in a menu" requirements).
- No API, backend, or persistence changes.
