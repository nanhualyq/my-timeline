## Context

Folder rows are rendered by `src/components/Sidebar.vue`, which linearizes global views, folder headers, nested feeds, and the uncategorized group into a single `items` list; folder and feed rows share the `feed-item` button styling. The folder row currently consists of a select button (name + unread badge) and a `RowMenu` dropdown. There is no expansion state anywhere in the app, and no persistence layer for UI state (no `localStorage` usage in `src/`). See proposal.md for motivation and `specs/feed-folders/spec.md` for the required behavior.

## Goals / Non-Goals

**Goals:**
- Per-folder expand/collapse driven by a leading icon toggle on the folder row.
- All folders collapsed at every launch; state is session-only.
- Selection and expansion are independent interactions.

**Non-Goals:**
- Persisting expansion state across restarts.
- Expanding/collapsing all folders at once (bulk toggle).
- Collapsing the global views or the uncategorized group.
- Any backend/API changes.

## Decisions

**1. Expansion state as a `Set<number>` of expanded folder ids in `Sidebar.vue`.**
An empty set means "all collapsed", which makes the default state free — no initialization loop, and a new folder is collapsed automatically. Alternatives considered: a `Record<folderId, boolean>` (defaults need care: missing key must map to collapsed) and a "collapsed set" (default-expanded unless opted out, inverting the required default). Component-local state is enough because expansion is purely presentational and resets on unmount, which is exactly the required per-launch reset; no `localStorage`.

**2. Toggle visibility of nested feeds by filtering the `items` computed, not by hiding rows with CSS.**
The `items` computed skips a folder's feeds when the folder is not expanded, so collapsed folders contribute no feed rows to the v-for (no hidden DOM, no unreachable buttons for keyboard/screen-reader users). Alternative considered: render everything and toggle with `v-show`/CSS — rejected because hidden-but-focusable rows would violate the "nested feeds hidden" behavior for assistive tech.

**3. Toggle is a separate `<button>` placed as the first child of `.folder-row`, outside the folder's select button.**
Clicking the toggle calls `toggleFolder(id)` with `.stop` propagation so it neither selects the folder nor triggers anything else; clicking the folder name continues to emit `select` only. The toggle carries `aria-expanded` and `aria-label` (e.g. "Expand folder"/"Collapse folder") so its state is announced. Alternative considered: making the whole row a disclosure control where clicking the name expands — rejected: the user chose icon-only toggling so browsing behavior stays unchanged.

**4. The toggle uses a rotating caret glyph (▸/▾) rather than an icon library.**
The project has no icon dependency; existing rows already use text glyphs (⚙, ★, ⋮). A scoped CSS `transition` on the caret gives cheap affordance without new dependencies.

## Risks / Trade-offs

- [Narrow sidebar leaves less room for name + badge + toggle + menu] → Toggle is a compact fixed-width glyph; folder name already uses `text-overflow: ellipsis`, which absorbs the lost width.
- [Empty folders show a toggle that reveals nothing] → Keep the toggle always visible for row-layout consistency; expansion of an empty folder is a harmless no-op (still spec-compliant — no requirement hides it).
- [Filtering `items` changes row keys/indices across toggles] → Rows are keyed by stable `key` strings (`folder-<id>`, `feed-<id>`), so Vue reuses DOM correctly; verify active-row highlighting still tracks selection after a toggle.
- [A folder selected while collapsed shows entries with no visual anchor in the sidebar] → Accepted: selection highlight stays on the folder row; the spec keeps selection and expansion independent.
