# Tasks: add-settings-dialog

## 1. Settings dialog component

- [x] 1.1 Create `src/components/SettingsDialog.vue` modeled on `FeedEditDialog.vue` (Teleport to body, overlay `@click.self` close, Escape via document `keydown`, `role="dialog" aria-modal="true"`, `busy` prop, `close`/`changed` emits, inline message area) and verify `npm run check` passes on the new file.
- [x] 1.2 Move the add-feed flow into the dialog verbatim (`newUrl`/`addError`/`addBusy`, `submitAdd`, `addFeed` import) with the form rendered inside the dialog, and verify in `npm run dev` that a valid URL adds a feed whose error/success state shows inside the dialog (spec: "Adding a feed from the Settings dialog").
- [x] 1.3 Move folder creation into the dialog using an in-dialog folder-name input replacing `prompt()`, wired to `createFolder` + the existing `changed` emit, and verify creating "Tech" makes it appear in the sidebar and an invalid/empty name surfaces an error in the dialog (spec: "Creating a folder from the Settings dialog").
- [x] 1.4 Move OPML import/export into the dialog (same `open`/`save` Tauri file dialogs, same `importOpml`/`exportOpml` calls), replacing `alert()` with the dialog's inline message area, and verify the import summary and export confirmation render inside the dialog (spec: "Import summary is reported in the Settings dialog").

## 2. Sidebar cleanup and settings entry point

- [x] 2.1 In `src/components/Sidebar.vue`, remove the add-feed `<form>`, the `.opml-actions` block, and the "+" button on the Folders header (header renders as a plain label like Uncategorized), add a single Settings control that emits `open-settings`, and verify `npm run dev` shows the first screen as: settings control → All entries → Starred → Folders/feeds list with no add input, no OPML buttons, no "+" (spec: "First screen shows navigation only").
- [x] 2.2 Remove add-feed/OPML state and imports that left the sidebar (`addFeed`, `createFolder`, `importOpml`, `exportOpml`, `open`, `save`, `newUrl`, `addError`, `addBusy`, `submitAdd`, `doImport`, `doExport`, `createFolderAction`) while keeping `runAction`/`actionError` for folder rename/delete, and verify `npm run check` passes with no unused-symbol errors.

## 3. App-level wiring

- [x] 3.1 In `src/App.vue`, add `settingsOpen` state, render `SettingsDialog` with `v-if` next to `FeedEditDialog`, connect `@changed="reload"`, guard mutual exclusivity (opening one closes the other), and verify the dialog opens from the sidebar, closes via Escape and overlay click without changing the current selection, and a successful action triggers a sidebar reload (specs: "Single settings entry point", "Escape closes the dialog").

## 4. Verification

- [x] 4.1 Run `npm run check` and `npm run build` and verify both succeed with no type or build errors.
- [x] 4.2 Walk every scenario in `openspec/changes/add-settings-dialog/specs/settings/spec.md` manually in `npm run dev` and confirm each passes (open dialog reveals all four actions; add feed success/error inline; import summary inline; folder created from dialog; exactly one sidebar settings control; Escape/overlay close preserves selection; first screen navigation-only; add-feed reachable via settings).
- [x] 4.3 Regression-check pre-existing flows in `npm run dev`: feed row menu (edit/refresh/remove), folder row menu (refresh/rename/delete), feed edit dialog, refresh all/one/folder, and OPML round-trip behavior unchanged.
