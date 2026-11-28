## Why

A WebDAV sync that only *receives* changes reports `nothing_changed`, because that outcome describes the document on the server rather than the local library. The frontend treats it as "no refresh needed", so feeds, folders, and marks imported from another installation stay invisible until the application is fully restarted. A reader who syncs on demand and sees nothing happen concludes the sync did not work.

## What Changes

- The sync status gains an explicit statement of whether the local library changed, separate from the outcome (`synced` / `created` / `nothing_changed` / `failed`), which keeps describing what happened to the remote document.
- A sync that only downloads and merges reports `applied = true` even when nothing needed uploading.
- The application refreshes the sidebar, entry counts, entry list, and the open entry after any sync — foreground or background, manual or automatic — that applied changes to the library.
- Status records written by earlier builds remain readable (the new field defaults to "nothing applied") and the frontend keeps refreshing for the outcomes it already understood.
- No change to what the sync document shares: entry bodies are still re-acquired by each installation, so a newly synced subscription shows up in the sidebar but its entries arrive through the existing refresh actions, not during the sync.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `webdav-sync`: the status must report whether the sync applied changes to the local library, and a sync that applied changes must make the view show them.

## Impact

- `src-tauri/src/sync.rs`: `SyncStatus` gains the field; the cycle sets it from the import summary for every outcome, including `nothing_changed`, `failed`-free paths, and the throttle/"already running" paths that replay a recorded status. Stored-status JSON stays compatible.
- `src/api.ts`: `SyncStatus` type gains the field.
- `src/lib/background-sync.ts`: `appliedChanges` decides from the new field, keeping the existing outcomes as a fallback.
- `src/components/SettingsDialog.vue`: "Sync now" refreshes the view on the same signal instead of on the outcome alone.
- Tests: Rust cycle tests for a download-only merge, the focus-throttle replay, and a local-only upload; frontend specs for the status-to-refresh decision and the dialog's `changed` emit.
