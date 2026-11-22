## 1. Expansion state in Sidebar

- [x] 1.1 Add a session-only `Set<number>` of expanded folder ids to `Sidebar.vue` (empty = all collapsed) with a `toggleFolder(id)` handler, and verify via `npm run check` that it type-checks
- [x] 1.2 Filter the `items` computed so a collapsed folder contributes no feed rows, and verify in the running app that a collapsed folder renders no nested feed DOM

## 2. Toggle control on the folder row

- [x] 2.1 Add a leading toggle button (caret glyph, fixed width) as the first child of `.folder-row`, before the folder's select button, with `.stop` propagation, `aria-expanded`, and an "Expand folder"/"Collapse folder" label, and verify clicking it does not change the sidebar selection
- [x] 2.2 Style the toggle and caret rotation (scoped CSS) so the row keeps name, unread badge, and menu control aligned, and verify the folder name still ellipsizes rather than overflowing

## 3. Verification against spec scenarios

- [x] 3.1 Run `npm run check` and confirm it passes
- [x] 3.2 Manually verify in `npm run dev:web`: all folders collapsed at launch; expanding shows only that folder's feeds; collapsing hides them while name and badge stay; clicking a folder name selects it without expanding; menu still offers Refresh folder/Rename/Delete; uncategorized group and global views unaffected
