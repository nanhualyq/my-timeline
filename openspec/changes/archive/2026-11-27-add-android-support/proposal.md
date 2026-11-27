## Why

The reader only exists as a desktop window, so it cannot be used where reading actually happens: on a phone in short bursts. The codebase is already close to portable — the Vue frontend talks to Rust only through Tauri commands, storage is a bundled SQLite file under the app data directory, and the Rust entry point already carries `#[cfg_attr(mobile, tauri::mobile_entry_point)]` — but the shell is desktop-only: a fixed three-column grid with no media queries, hover-revealed row controls, a keyboard-first interaction model, and OPML commands that read filesystem paths.

This change ships an Android build of the same application with its own local library, so the reading loop works on a phone without introducing a server, an account, or a sync protocol.

## What Changes

- Add an Android build target to the existing Tauri project, producing an installable APK from the same Vue frontend and Rust backend.
- Introduce narrow-viewport behavior driven by available width: below a breakpoint the sidebar becomes an overlay drawer, the entry list and the reading pane become two stacked screens instead of side-by-side columns, and Android back navigation pops reader → list → drawer before leaving the app.
- Add touch affordances where the desktop UI relies on hover or on tooltips: row actions that appear only on hover become tap-reachable, tap targets meet a minimum touch size, and sticky hover styling is suppressed.
- Decouple OPML import and export from filesystem paths so they work with the platform file picker on Android, where the picker returns a content URI rather than a path. Desktop behavior is unchanged.
- Scope keyboard bindings to hardware-keyboard availability: shortcuts stay live on desktop and on Android devices with a physical keyboard, while the help overlay is not offered on touch-only narrow layouts.
- Add Android developer and packaging commands alongside the existing desktop ones.
- Out of scope: synchronization of any kind between devices (subscriptions, read state, stars), Play Store publishing and signing, periodic background refresh, push, and any change to desktop rendering or desktop behavior.

## Capabilities

### New Capabilities

- `mobile-shell`: the application runs as an Android app whose narrow layout presents one pane at a time — a feed drawer, an entry list, and a reading screen — with drawer and back navigation, touch-sized controls, safe-area handling, and a refresh-on-resume reading loop.

### Modified Capabilities

- `dev-build-workflow`: adds Android initialization, development, and packaging commands that build and run the application on a device or emulator, without changing the existing desktop commands.
- `keyboard-navigation`: the keyboard cursor and key bindings apply when a hardware keyboard is present; on touch-only narrow layouts the shortcut help overlay is not offered and no action depends on a keystroke.
- `opml-interchange`: import and export work through the platform's file picker, so the Android picker's content URIs are accepted as well as filesystem paths, with the reported import and export outcomes unchanged.

## Impact

- Frontend: `src/App.vue` layout, `src/components/Sidebar.vue` (drawer form), `EntryList.vue` (row controls, tap targets), `ReadingPane.vue` and the dialogs (viewport sizing, dismissal), `src/lib/keymap.ts` (activation conditions), a new breakpoint/navigation composable, and `SettingsDialog.vue` (OPML picking).
- Rust: `import_opml` and `export_opml` in `src-tauri/src/lib.rs` must accept content instead of only a path, and `src-tauri/src/opml.rs` is unaffected in logic.
- Build: new Android configuration in `src-tauri/tauri.conf.json` (minimum SDK, version code, adaptive icons), generated `src-tauri/gen/android` project, mobile capability window scoping in `src-tauri/capabilities/`, a new `aarch64-linux-android` (and emulator) Rust target, and an Android SDK/NDK toolchain requirement for developers.
- No schema or storage change: each install keeps its own `reader.db`; existing tables and migrations are reused as-is.
