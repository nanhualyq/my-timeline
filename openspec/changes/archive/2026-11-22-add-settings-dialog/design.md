# Design: add-settings-dialog

## Context

The sidebar (`src/components/Sidebar.vue`) currently owns three setup flows inline at the top of its column: the add-feed `<form>`, the OPML import/export button pair, and the "+" control on the Folders group header (folder creation via `prompt()`). Import/export results surface through `alert()`; add errors render in `<p>` elements above the feed list. The app already has a working modal pattern: `FeedEditDialog.vue` — a `Teleport to="body"` overlay with `@click.self` close, Escape handled by a document `keydown` listener, `role="dialog" aria-modal="true"`, opened from app-level state in `App.vue` (`editingFeed`). See proposal.md for motivation and specs/settings/spec.md for the behavior contract.

## Goals / Non-Goals

**Goals:**

- One Settings dialog hosting add feed, create folder, and OPML import/export, opened from a single sidebar control.
- Sidebar first screen dedicated to navigation (global views, folders, feeds).
- Reuse the existing dialog pattern so the change stays frontend-only with no new dependencies.

**Non-Goals:**

- Regrouping or renaming the actions inside the dialog — the user explicitly deferred finer grouping until there are more settings.
- Moving rename/delete folder or feed edit/remove — these already live in row menus or their own dialog and are not first-screen clutter.
- Backend, `src/api.ts`, or behavior changes to the four actions themselves (validation, merge-only import, error text).
- A persistent settings model (nothing here needs to be stored; the dialog is pure UI).

## Decisions

**1. New `SettingsDialog.vue` modeled on `FeedEditDialog.vue` (Teleport overlay, Escape + overlay-click close).**
Alternative considered: keep `prompt()`/`alert()` for the folder name and import summary. Rejected — the spec requires messages to be visible *while the dialog is open*, and stacking `prompt`/`alert` on top of a modal is both fragile and inconsistent with the dialog. The dialog therefore carries its own folder-name text input and an inline message area for errors and import/export summaries.

**2. Dialog open state lives in `App.vue` (`settingsOpen = ref(false)`), not inside `Sidebar`.**
The overlay must cover the whole three-pane layout, so it has to be mounted at layout level. This mirrors how `editingFeed` already works: `Sidebar` emits nothing for it; instead `Sidebar` exposes a settings control that App reacts to (e.g. `@open-settings`), and App renders `<SettingsDialog v-if="settingsOpen" …>` next to `FeedEditDialog`. Alternative considered: state inside `Sidebar` with the dialog teleported from there — rejected because it splits dialog lifecycle across components for no gain.

**3. Move the four action implementations out of `Sidebar.vue` into `SettingsDialog.vue`.**
`submitAdd`, `createFolderAction`, `doImport`, `doExport`, plus `addError`/`actionError`/`addBusy` state and the `addFeed`/`createFolder`/`importOpml`/`exportOpml`/`open`/`save` imports relocate wholesale, keeping identical API calls and `String(e)` error rendering. The dialog emits `changed` (same payload-less contract as today) so `App.vue` reuses its existing `@changed="reload"` wiring. `Sidebar` keeps row menus, `prompt`/`confirm` for rename/delete (unchanged, out of scope), and all navigation.

**4. Single settings control at the top of the sidebar column.**
A compact "Settings" button (gear glyph + label) sits where the add form is today — one row instead of a form plus two buttons plus a "+". Placement at the top is an assumption recorded here; moving it (e.g. to the sidebar bottom) is a one-line template change and does not affect the specs, which only require *exactly one* entry point. The Folders group header loses its "+" and renders as a plain label like the Uncategorized header.

**5. Busy/`disabled` propagation unchanged.**
The dialog receives the existing `busy` prop from App (same one the sidebar form uses today) and disables its actions while a refresh is running, preserving current behavior.

## Risks / Trade-offs

- [Add-feed or OPML flows regress during the code move] → Move the functions verbatim first (same API calls, same error strings), then only change where messages render; the existing spec scenarios for these actions double as acceptance checks.
- [Inline import summary replaces `alert()` — a visible behavior change] → Intended and covered by the new spec scenario; summary text keeps the same content (`X added, Y skipped, Z folder(s) created` / `Exported N feeds`).
- [Sidebar feels emptier; users may not find "Add feed" where they expect] → The settings control is labeled and placed in the prime spot the form used to occupy; spec requires add-feed to remain reachable in one click.
- [Two modals (feed edit, settings) could ever stack] → `App.vue` never opens both: opening settings can close `editingFeed` and vice versa (guard in the open handlers).
- [Escape listener collision if both dialogs mount] → Same pattern as today's single dialog; `v-if` ensures only one document `keydown` listener exists at a time.

## Migration Plan

Pure frontend refactor with no data or API changes; ship in one commit and revert that commit to roll back. No feature flag needed.

## Open Questions

None — dialog placement, contents, and interaction are decided above; finer grouping inside the dialog is explicitly deferred to a later change by the user.
