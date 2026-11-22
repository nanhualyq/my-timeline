# Proposal: add-settings-dialog

## Why

The sidebar's first screen is prime reading real estate, yet it currently opens with three rarely used setup controls — the add-feed URL form, the two OPML import/export buttons, and the "+" new-folder button — pushing the views users actually click every day (All entries, Starred, the folders list) further down. These subscription-management actions are needed mostly during initial setup, so they should be gathered into one secondary panel (a Settings dialog) opened from a single entry point, leaving the sidebar dedicated to navigation. One consolidated Settings home also gives future infrequent options a natural place to land before we regroup anything.

## What Changes

- Add a **Settings dialog** (a modal, following the existing `FeedEditDialog` pattern) that hosts:
  - the add-feed-by-URL form,
  - the create-folder action,
  - OPML import and OPML export.
- Add a **single Settings entry control** in the sidebar that opens the dialog; the dialog closes via Escape or overlay click, and its errors and busy states are reported inside the dialog.
- **Remove** the always-visible add-feed form, the OPML Import/Export buttons, and the folder "+" button from the sidebar body.
- The sidebar's first screen then starts directly with the global views (All entries, Starred) and the folders list.
- No behavior changes to the actions themselves: add feed, create folder, and OPML import/export keep their current validation, merge, error-reporting, and reload semantics — only their entry point moves.
- No changes to feed/folder row menus, the feed edit dialog, the entry list, backend commands, or `src/api.ts`.

## Capabilities

### New Capabilities

- `settings`: The Settings dialog as the home for infrequent subscription-management actions (add feed, create folder, OPML import/export), its single sidebar entry point, and the requirement that these controls no longer occupy the sidebar's first screen.

### Modified Capabilities

- (none) — The existing `rss-reader`, `feed-folders`, and `opml-interchange` requirements describe what each action must do, not where its control lives; they remain true after this change. Placement is new spec-level behavior and lives in the new `settings` capability.

## Impact

- **Frontend only**: `src/components/Sidebar.vue` (remove the add form, OPML actions, and folder "+" button; add the Settings entry), a new `src/components/SettingsDialog.vue`, and `src/App.vue` (dialog open/close state and reload wiring, mirroring how `editingFeed` is handled today).
- No Rust/`src-tauri` changes, no `src/api.ts` changes, no new dependencies.
- Existing specs are unaffected; `openspec/specs/` stays as is apart from the new `settings` capability after archive.
