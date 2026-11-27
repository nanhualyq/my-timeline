## 1. Android Toolchain and Shell

- [x] 1.1 Document the Android prerequisites (SDK, NDK, JDK 17+, `aarch64-linux-android` device target, `x86_64-linux-android` if an emulator is used) in `README.md`, and verify a clean-machine reader can follow the list without consulting external docs
- [x] 1.2 Add `android:init`, `android:dev`, and `android:build` scripts to `package.json` wrapping the Tauri CLI, and verify `npm run android:build` exits non-zero with a message naming the missing prerequisite when the SDK is absent
- [x] 1.3 Add the `bundle.android` block (`minSdkVersion: 24`, `versionCode: 1`) and adaptive icon entries to `src-tauri/tauri.conf.json`, and verify `npm run build:app` still produces the Windows artifact unchanged
- [x] 1.4 Run the Android initialization once and verify `src-tauri/gen/android` exists, builds, and is committed (including `MainActivity`, manifest, and icons)
- [x] 1.5 Build and install a debug APK on a device and verify the existing three-column UI loads with a working IPC call (feeds list rendered, no console errors)
- [x] 1.6 Spike: cross-compile a minimal `reqwest` + `rustls` request for `aarch64-linux-android` and verify it builds and completes a fetch on device; if the aws-lc provider fails, switch to the `ring` provider and record the choice in `design.md`
- [x] 1.7 Spike: verify whether the Android back action reaches the WebView as `popstate`; record the answer and, if it does not, plan the `MainActivity` forwarding patch (see 5.3)

## 2. Layout Seam and Narrow Shell

- [x] 2.1 Add `src/lib/viewport.ts` with `NARROW_MAX_WIDTH = 800`, a `matchMedia` listener that sets `data-layout` on the root element, and a `useLayout()` composable; verify with a unit test that crossing the width toggles the attribute and unmounting removes the listener
- [x] 2.2 Replace the fixed grid in `src/App.vue` with wide rules that keep the current three columns and narrow rules that render one pane, and verify the existing Vitest suite passes unchanged
- [x] 2.3 Implement the narrow sidebar drawer (toggle in the list header, overlay with backdrop, close on backdrop tap and on selection) and verify with a narrow-mode component test that selecting a feed closes the drawer and loads that selection
- [x] 2.4 Implement the narrow reading screen (opens on row selection, restores list scroll position and selected row on close, offers back/read-toggle/open-link controls) and verify with a component test that opening and returning preserves the selected row
- [x] 2.5 Reconcile state on breakpoint changes (drawer closes, reader stays open when an entry is selected) and verify with a test that switching narrow → wide shows three columns with the same open entry
- [x] 2.6 Make the list header usable at phone width by wrapping the view tabs and the `Refresh all` / `Mark N read` actions instead of clipping, and verify at a 360 px viewport that both actions are reachable

## 3. Touch Affordances

- [x] 3.1 Make `.mark-above` always visible and tappable on narrow layout while keeping it hover-revealed on wide, and verify with a test that a narrow row exposes the batch-mark control without hover
- [x] 3.2 Gate hover styling (`EntryList`, `Sidebar`, `RowMenu`, `ReadingPane`) behind `@media (hover: hover) and (pointer: fine)` and verify a tapped row does not retain the hover appearance in a device run
- [x] 3.3 Raise narrow-mode touch targets to at least 44x44 CSS px for the star, row menu, folder caret, and batch controls, and verify with a component test asserting computed sizes and with a device tap pass
- [x] 3.4 Replace `title`-only labelling on narrow with `aria-label` (or a visible label where the icon is ambiguous) for star, row menus, folder caret, settings, and refresh controls, and verify each control has an accessible name in a component test
- [x] 3.5 Confirm the fixed 7.5 rem row still fits the enlarged controls at 360 px with a large system font, and verify cover, title, excerpt, feed label, and time do not overlap adjacent rows

## 4. Dialogs and System Chrome

- [x] 4.1 Make settings, feed-edit, folder, and keyboard-help dialogs fit phone widths and scroll internally when taller than the viewport, and verify no control is clipped at 360x640
- [x] 4.2 Add safe-area and status-bar handling plus on-screen-keyboard avoidance on Android so headers, list footers, and the focused add-feed input stay visible, and verify on a device with gesture navigation and with the keyboard raised
- [x] 4.3 Make confirmation requests (feed and folder removal) usable by touch with tap-able accept/cancel and system back dismissal, and verify the removal flow on a device without a physical keyboard

## 5. Back Navigation

- [x] 5.1 Add `src/lib/back-stack.ts` that pushes one history entry per layer and unwinds in the order dialog → drawer → reader → leave, and verify with unit tests covering each layer and the empty stack
- [x] 5.2 Wire the back stack into the drawer, reading screen, and dialogs, and verify with narrow-mode component tests that back closes one layer per action and leaves the app at the top level
- [x] 5.3 Not needed, deliberately skipped: the 1.7 spike showed the system back action does reach the WebView as `popstate` (drawer closed on back, activity stayed resumed, second back left the app), so `MainActivity.kt` stays unpatched
- [x] 5.4 Verify on desktop that back-related listeners do not interfere with existing keyboard navigation or dialog Escape handling (`npm test` plus a manual smoke)

## 6. OPML and Resume Refresh

- [x] 6.1 Spike: read an OPML file through the Android picker's returned reference and verify whether `@tauri-apps/plugin-fs` can open it; record the outcome and pick the IO helper described in design D5
- [x] 6.2 Change the two command wrappers in `src-tauri/src/lib.rs` to content-only (`import_opml(xml)`, `export_opml() -> { xml, feeds_exported }`), and verify `cargo test` passes with the OPML tests exercising `opml.rs` directly
- [x] 6.3 Move file picking, reading, and writing into `SettingsDialog.vue` (and the corresponding `src/api.ts` signatures), and verify a desktop OPML round-trip exports and re-imports the same folders and feeds
- [x] 6.4 Add Android OPML coverage and verify on device that importing a file shared to the picker creates its folders and feeds and reports the same counts as desktop
- [x] 6.5 Implement resume refresh on narrow layout (visibility trigger, 60 s threshold, selection-mapped command, shared `busy` state) and verify with a test that a hidden-then-visible document refreshes the current selection and an in-app dialog does not
- [x] 6.6 Verify no refresh is scheduled while the app is in the background and that a failing feed on resume reports through the existing error path without blocking the list

## 7. Verification

- [x] 7.1 Run `npm test` and confirm the full suite passes, including the new narrow-mode, back-stack, and OPML tests
- [x] 7.2 Run `npm run check` and verify zero type errors across the shared frontend
- [x] 7.3 Run `cargo test` in `src-tauri` and verify the storage, reader, and OPML suites pass
- [x] 7.4 Run the desktop smoke pass (three columns unchanged, keyboard cursor and bindings, OPML round-trip, dialogs) and confirm no visual or behavioral regression
- [x] 7.5 Walk every scenario in `specs/mobile-shell/spec.md` on a device and record the result, including drawer, reading screen, back unwinding, touch targets, insets, and resume refresh
- [x] 7.6 Add a test pinning that no visible affordance advertises keyboard shortcuts on a touch-only narrow layout, and verify it passes
- [x] 7.7 Run `openspec validate add-android-support --strict` and verify the change validates before archiving
