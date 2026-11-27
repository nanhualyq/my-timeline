## Context

See `proposal.md` — Why. Constraints that shape the approach, from the current codebase:

- One Vue SPA (`src/App.vue`) renders a fixed 3-column grid (`minmax(200px,260px) minmax(260px,360px) 1fr`) and the codebase contains no `@media` rule at all. `App.vue` owns `selection`, `view`, `selectedEntryId`, `selectedEntry`, paging, and the keyboard cursor; components are presentational and communicate through props/events.
- `src/api.ts` is the only seam to Rust. Nothing in the frontend reads files; only `SettingsDialog.vue` calls `open`/`save` from the dialog plugin and passes the returned string to `importOpml(path)` / `exportOpml(path)`.
- `src-tauri/src/opml.rs` is already IO-free: `import_opml(conn, xml: &str)` and `export_opml(conn) -> Exported { xml, feeds_exported }`. Only the two Tauri command wrappers in `lib.rs` touch `std::fs`.
- `lib.rs` already uses `#[cfg_attr(mobile, tauri::mobile_entry_point)]`; the database path comes from `app.path().app_data_dir()` with a `-dev` suffix under `debug_assertions`, so mobile and desktop cannot share a library even by accident.
- Interaction assumptions that do not survive touch: `.mark-above` is `opacity: 0; pointer-events: none` until `.entry:hover`; `.entry:hover` background sticks after a tap; several controls are labelled only by `title=`; star/menu padding is well under 44 CSS px.
- Tests run in jsdom via Vitest against a `FakeBackend` that reimplements the paging contract, so layout work must stay testable without a real viewport.
- Installed tooling today: JDK 21, cargo with only `x86_64-pc-windows-msvc`; no Android SDK/NDK, no `src-tauri/gen/android`.

## Goals / Non-Goals

**Goals:**

- One component tree, one build, two layouts: narrow and wide differ by state and CSS, not by duplicated components.
- Desktop rendering stays pixel-identical; the existing Vitest suite keeps passing untouched.
- Every narrow-viewport behavior in `specs/mobile-shell/spec.md` is exercisable in jsdom.
- OPML import/export becomes platform-agnostic without adding a second code path per platform.

**Non-Goals:**

- Any sync, account, or server work; the two installs are independent libraries.
- Play Store publishing, release signing, and store metadata.
- Redesigning the reading pane's typography or the entry row's content rules for mobile.
- Background (app-stopped) refresh; resume-time refresh only.
- Reworking the keyboard state machine, which already suppresses bindings while typing or while a dialog blocks input.

## Decisions

### D1: Breakpoint state lives in TS, layout lives in CSS

A single constant and one root attribute decide the layout: `src/lib/viewport.ts` exports `NARROW_MAX_WIDTH = 800` and applies `document.documentElement.dataset.layout = "narrow" | "wide"` from a `matchMedia` listener, plus a Vue composable (`useLayout()`) exposing `narrow`. All layout CSS keys off `[data-layout="narrow"]` instead of duplicating the number in `@media`.

- Why 800: the grid's own minimum is ~780 px, and the desktop window's `minWidth` is 900, so no desktop window can ever fall into narrow mode. Tablets at ≥800 px get the three-column layout.
- Why not `@media` alone: the numeric breakpoint would then exist twice and drift, and the narrow layout needs JS state anyway (drawer, reader screen, back). CSS alone also cannot unmount the reading pane.
- Why not platform detection (`isAndroid()`): a desktop window narrowed to 700 px should behave like the phone, and platform branching makes the mobile path untestable in jsdom.
- Alternatives rejected: separate mobile component tree (duplicate layouts, two test sets); container queries (no benefit, `:root` sizing is what matters).

### D2: Narrow navigation is two pieces of state in `App.vue`

`drawerOpen: boolean` and `readerOpen: boolean` join the existing refs. Wide layout ignores both (three columns always shown); narrow layout renders exactly one of list/drawer/reader.

- `selectEntry()` sets `readerOpen = true` on narrow; `closeReader()` sets it false and leaves `selectedEntryId`/`selectedEntry` intact so the row stays marked.
- Crossing the breakpoint reconciles rather than resets: wide → narrow keeps `readerOpen` true if an entry is open (the reader is what the reader was looking at); narrow → wide closes the drawer and shows the reader as the third column.
- Selection of a feed/folder closes the drawer and leaves the reader, matching desktop where selection does not clear `selectedEntry` but does reload the list.

### D3: Back navigation through webview history, with an Android fallback

A small `src/lib/back-stack.ts` pushes one history entry per overlay layer (dialog, drawer, reader) and unwinds on `popstate`, in that priority order. If the Android Activity does not forward back to the WebView (unverified — see Risks), the generated `src-tauri/gen/android/.../MainActivity.kt` is patched to forward `onBackPressed` into the WebView.

- Why history over a plugin: no new native dependency, no capability change, and the same mechanism makes narrow mode testable in jsdom (`window.history` exists, `PopStateEvent` is synthesizable).
- Alternative rejected: a community Android back-button plugin (extra dependency, extra capability surface, still needs per-layer logic).

### D4: Touch affordances are additive, not a redesign

- Hover-revealed `.mark-above` becomes always-visible on narrow, keeping its floating position (it already reserves no column) and gaining padding to reach a 44 px hit area.
- Hover styling is wrapped in `@media (hover: hover) and (pointer: fine)` so taps do not leave rows looking hovered; `:active` and `.active` states carry the touch feedback.
- Every `title=`-only control gains an `aria-label` (some already have one) so the purpose survives without a tooltip; visible labels are added only where the icon is genuinely ambiguous.
- Touch targets: star, row-menu trigger, folder caret, and batch controls get `min-width/min-height: 44px` on narrow. The row's `7.5rem` height is rem-based and already scales with the system font; the 44 px controls fit inside it, so the row does not grow.
- The list header's two buttons (`Refresh all`, `Mark N read`) wrap into two rows on narrow instead of clipping.

### D5: OPML IO moves to the frontend; Rust keeps content-only commands

`import_opml` becomes `import_opml(xml: String)` and `export_opml()` returns `{ xml, feeds_exported }`. `SettingsDialog.vue` picks a file, reads it to text, and passes the string; on export it writes the returned string to the picked destination. `src-tauri/src/opml.rs` is unchanged.

- Choosing the IO helper is settled, and verified on a device: `@tauri-apps/plugin-fs` reads the picker's reference and writes through the create-document reference. The dialog plugin already does the two things the Android case needs — it adds the picked path to the filesystem scope (so no broad `fs:scope` is declared) and it copies the picked document into the app sandbox by default (`fileAccessMode: "copy"`), which is what makes a `content://` document readable at all. Writing through the save picker's reference also works, so no Kotlin `ContentResolver` fallback is needed.
- The picker filter, in contrast, needed platform-specific values. Android filters by MIME type and ignores extensions, so `extensions: ["opml", "xml"]` matched nothing: a real `.opml` file arrived as `application/octet-stream` ("BIN file") and could not be selected at all. On Android the filter now lists `text/xml`, `application/xml` and `application/octet-stream`; desktop keeps the extension filter.
- Alternative rejected: keeping the path-based command and resolving content URIs in Rust — Rust has no `ContentResolver` access without a JNI bridge, which is the fallback we would rather avoid.

### D6: Resume refresh reuses the existing commands, triggered by visibility

On narrow layout, a `visibilitychange` listener refreshes when the document becomes visible again after at least 60 s hidden and only when the previous transition was a real background (not an in-app dialog). It maps the current selection to the existing commands: `refreshFolder` for a folder, `refreshFeed` for a feed, `refreshAll` for the global views, sharing the existing `busy` flag and error reporting.

- Why `visibilitychange` and not a native lifecycle event: it is the one signal available identically in the WebView, jsdom, and desktop; no plugin, no Android-specific code.
- Why a 60 s threshold: opening the app switcher or the notification shade must not trigger fetches.
- Alternative rejected: WorkManager background refresh — out of scope per the proposal, and it would need a native plugin.

### D7: Keyboard handling needs no code change

Listeners stay unconditionally registered. `lib/keymap.ts` already ignores events from editable targets and `lib/keyboard-blocks.ts` already suppresses bindings while a dialog is open, so an on-screen keyboard cannot trigger bindings and a hardware keyboard on a phone works as on desktop. The only narrow-specific change is that nothing advertises keys — there is no visible help trigger today (`?` only), so the requirement is satisfied by leaving it that way and pinning it with a test.

### D8: Android project generated and committed, desktop config untouched

`npm run android:init` wraps `tauri android init`; `npm run android:dev` and `npm run android:build` wrap `tauri android dev|build --apk`. `tauri.conf.json` gains a `bundle.android` block (`minSdkVersion: 24`, `versionCode: 1`) and adaptive icon entries; `app.windows` is left as-is since it is ignored on mobile. The generated `src-tauri/gen/android` project is committed (it carries `MainActivity` and the manifest). `capabilities/default.json` keeps targeting the `main` window label, which mobile also uses, so no capability split is needed; its `$schema` reference is a developer-experience detail, not a runtime constraint.

### D9: Tests get a viewport seam

`window.matchMedia` is missing in jsdom, so the test setup stubs it and `useLayout()` reads through an injectable `matchMedia` function; a helper sets the layout for a test. New narrow-mode component tests mount `App.vue` against `FakeBackend` and assert drawer/reader/back behavior and the presence of touch controls; existing tests keep mounting in wide mode.

## Risks / Trade-offs

- [Android back may not reach the WebView, so `popstate` never fires] → verify in the first device run; if it does not, patch the generated `MainActivity.kt` to forward the back gesture; the JS layer already models layer order, so only the trigger changes.
- [The file plugin may not open `content://` URIs from every provider] → spike against the device's own picker early (task 6); fallbacks in D5 are ordered by cost.
- [rustls' aws-lc provider may fail to cross-compile to `aarch64-linux-android`] → prove it in the toolchain spike with a trivial build; fall back to the `ring` provider or `native-tls` if needed.
- [Enlarging touch targets can break the fixed 7.5 rem row] → assert row height and meta-line visibility at narrow width in a component test, adjust height on narrow if it clips.
- [jsdom has no layout engine, so "fits on screen" claims cannot be tested there] → treat jsdom as structural coverage and verify insets, keyboard overlap, and real tap sizes on a device; the design keeps those behaviors in CSS with no JS logic to unit-test.
- [The breakpoint can move under a running app (rotation, window resize) with a dialog open] → reconcile state in one place on breakpoint change instead of in each component; dialogs are layout-independent.
- [`system-proxy` in the reqwest feature set is Windows/macOS only] → Android ignores Wi-Fi proxy settings; acceptable for this change, documented rather than worked around.
- [A second install is a second library with no merge path] → accepted by the proposal; OPML export/import is the documented migration path until a sync change exists.
- [Committing `gen/android` adds Gradle files to review surface] → keep generated files generated: only `MainActivity` (if patched), the manifest, and icons are edited by hand.

## Migration Plan

1. Toolchain and build: add Android scripts and config, generate `gen/android`, verify a debug APK installs and opens with the existing UI on a device.
2. Layout: breakpoint composable, narrow CSS, drawer, reader screen, paged list at phone width.
3. Touch affordances and dialogs, then back navigation.
4. OPML IO move (frontend change plus the two Rust wrapper signatures) and the resume refresh.
5. Verification: full Vitest and `cargo test` run plus desktop manual smoke (three columns unchanged, keyboard cursor, OPML round-trip) and a device pass through the mobile-shell scenarios.

Rollback: steps 2–4 are additive and reversible per decision; the one signature change (D5) is confined to the two command wrappers and `SettingsDialog.vue`, so reverting them restores path-based OPML if the SAF path proves unworkable.

## Open Questions

- Whether the first target set is arm64 devices only or arm64 plus an x86_64 emulator image; it only affects which Rust targets developers install.
- Whether the resume refresh should also fire on the first foreground after launch (currently assumed not, since the app loads fresh state anyway).
- Whether a tablet in landscape should keep the three-column layout or get a wider two-pane layout; deferred until the three-column tablet rendering is seen on a device.
