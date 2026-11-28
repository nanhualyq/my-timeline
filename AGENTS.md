# AGENTS.md

Notes for agents working in this repository.

## What this is

A Tauri 2 + Vue 3 + TypeScript RSS reader. The Rust side (`src-tauri/src`) owns
SQLite, fetching, sanitizing, and sync; the Vue side (`src`) owns layout, input,
and orchestration. Behavioral specs live in `openspec/specs`; changes are planned
in `openspec/changes`.

## Verifying behavior — do not verify by screenshot

Screenshots tell you a window rendered, nothing about what the app did. Both
targets can be driven at the DOM level instead. Prefer this over clicking
guessed pixel coordinates: a wrong guess looks exactly like a broken feature.

Shared CDP helper used below (Node 22+ ships a global `WebSocket`):

```js
// node eval.mjs <wsUrl> <expression>
const [wsUrl, expression] = process.argv.slice(2);
const ws = new WebSocket(wsUrl);
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
ws.send(JSON.stringify({
  id: 1,
  method: "Runtime.evaluate",
  params: { expression, returnByValue: true, awaitPromise: true },
}));
const reply = await new Promise((resolve) => {
  ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === 1) resolve(m); };
});
console.log(JSON.stringify(reply.result, null, 2));
```

### Android: emulator + adb + CDP (verified, primary route for behavior)

The frontend is shared with desktop, and debug builds expose the WebView to the
DevTools protocol, so this is the cheapest way to exercise real behavior —
including real touch gestures, which go through the browser's gesture
arbitration that jsdom does not simulate.

```bash
export ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"
export NDK_HOME="$ANDROID_HOME/ndk/30.0.16248370"
export PATH="$ANDROID_HOME/platform-tools:$PATH"

"$ANDROID_HOME/emulator/emulator" -avd Medium_Phone_API_37.0 -read-only &   # if none is running
npx tauri android build --apk --debug        # debug allows cleartext, for local fixtures
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
adb shell am start -n com.mytimeline.app/.MainActivity

PID=$(adb shell pidof com.mytimeline.app | tr -d '\r')
adb forward tcp:9333 localabstract:webview_devtools_remote_$PID
curl -s http://127.0.0.1:9333/json     # -> webSocketDebuggerUrl

node eval.mjs "$WS" "document.querySelector('.list-header').innerText"
```

- `Runtime.evaluate` for assertions and for clicking known elements
  (`element.click()`); `Input.dispatchTouchEvent` / `dispatchMouseEvent` when the
  real input pipeline matters (touch gestures, scrolling, hover).
- Read coordinates from the DOM (`getBoundingClientRect()`) rather than from a
  screenshot; the viewport is in CSS pixels (411 x 914 on the default AVD).
- A local fixture server is reachable from the emulator at `http://10.0.2.2:<port>`
  (the host's loopback). Release APKs forbid cleartext, debug builds allow it.
- The emulator has network access; a host-side `python -m http.server` is the
  deterministic source of a feed.

### Desktop: `npm run dev`, driven by hand or by CDP

```bash
npm run dev        # debug build, own data dirs (runs beside a release install)
```

The dev webview keeps WebView2's own DevTools (F12 / right-click → Inspect). It
can also be driven over CDP like the Android build — point WebView2 at a port and
read `webSocketDebuggerUrl` from `/json/list`:

```bash
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222 npm run dev
curl -s http://127.0.0.1:9222/json/list   # -> webSocketDebuggerUrl, then eval.mjs above
```

This works while a release install is running, because debug desktop builds
override their app directories (`$DATA/com.mytimeline.app-dev`,
`$LOCALAPPDATA/com.mytimeline.app-dev`) in `src-tauri/src/lib.rs`.

**`0x8007139F` (`ERROR_INVALID_STATE`) when creating the webview means two
instances shared one WebView2 user data folder with different browser
arguments.** WebView2 keys one browser process per user data folder and treats
`AdditionalBrowserArguments` as global state for it, so a second environment
whose arguments differ — adding `--remote-debugging-port` is enough — fails
outright instead of opening a port. Before the directory override existed, dev
and release both resolved the folder to `%LOCALAPPDATA%\com.mytimeline.app`
(derived from the bundle identifier, which `tauri dev` does not change), so this
happened whenever the production app was left running. If it reappears, the
culprit is a live `msedgewebview2` browser process on the same folder, not the
port itself: quit the other instance, or give this one a separate folder with
`WEBVIEW2_USER_DATA_FOLDER`.

Notes for this app:

- `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` is *appended to* (and wins over) the
  arguments wry sets via `additionalBrowserArgs`, so the environment variable is
  the less invasive way to add the flag. The config route works too, but it
  replaces wry's defaults — include
  `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection` yourself.
- `dataDirectory` in `app.windows[]` is ignored in tauri 2.12: the
  `From<&WindowConfig> for WebviewAttributes` conversion never maps it. Use
  `appDirectoriesOverride` (as this app does), `WEBVIEW2_USER_DATA_FOLDER`, or
  `WebviewWindowBuilder::data_directory`.
- Two webview instances with *identical* arguments (plain `npm run dev` next to a
  release install) coexist fine; only differing arguments fail.

### Picking the layer — all rows verified against WebView2 runtime 158.0.4370.45

Each tool only proves its own layer: a green jsdom test says nothing about
WebView2, and CDP says nothing about the native shell or a release build.

| Tool | Proves | Cost | Platform |
| --- | --- | --- | --- |
| `cargo test` | db/fetch/sync logic on real SQLite (`db::open_memory`) | none | all |
| `npm test` (jsdom + `vi.mock("@tauri-apps/api/core")`) | component logic and events | none | all |
| `mockIPC` (`@tauri-apps/api/mocks`) | the real `invoke` → `__TAURI_INTERNALS__` path, argument serialization | none | all |
| Vite + headless Edge + injected `__TAURI_INTERNALS__` | real Chromium layout, CSS, media queries, hit testing | Edge installed | all |
| CDP (`--remote-debugging-port`) | real app, real backend, whole protocol | none | desktop, per app |
| tauri-driver + msedgedriver | real app through W3C WebDriver (elements, clicks, `execute`) | driver version must equal the WebView2 runtime | Win/Linux |
| adb + CDP / `uiautomator dump` | Android, real touch / native layer | emulator | Android |

Default set: `cargo test`, `npm test`, and CDP on the real app. Add tauri-driver
only when a WebDriver client or a platform matrix is wanted.

- Use jsdom for components and CDP for behavior. Reach for a real browser only
  when layout is what you are checking: jsdom performs no layout, so
  `getBoundingClientRect()` and `matchMedia` results there are fiction.
- Prefer `mockIPC` over `vi.mock("@tauri-apps/api/core")` when the command name
  or the argument shape is what could be wrong — it goes through the real
  serialization. Neither sees Rust-side serde, which only the two real-app rows do.
- tauri-driver (installed as `~/.cargo/bin/tauri-driver.exe`; `msedgedriver.exe`
  must match the WebView2 runtime version, from
  `https://msedgedriver.microsoft.com/<version>/edgedriver_win64.zip`). It takes
  `tauri:options.application`, then plain W3C calls work:

  ```bash
  tauri-driver --native-driver %TEMP%/wd/msedgedriver.exe --port 4444
  # POST /session   {"capabilities":{"alwaysMatch":{"tauri:options":{"application":"…/target/debug/my-timeline.exe"}}}}
  # POST /session/{id}/execute/sync  {"script":"return document.querySelector('.list-header').innerText","args":[]}
  # POST /session/{id}/element      {"using":"css selector","value":".list-header"}  then GET …/text
  # DELETE /session/{id}
  ```

  Build the target with `npx tauri build --debug --no-bundle` (it embeds the
  frontend, so the driver can launch it without a dev server). Only webview
  content is reachable — native menus and file dialogs are not. macOS has no
  WebDriver path.
- The web build in a real browser is the cheapest layout harness: run
  `npm run dev:web`, launch
  `msedge --headless=new --remote-debugging-port=9333 --user-data-dir=<temp>` on
  `http://localhost:1420`, then inject a stub before the app boots:

  ```js
  // CDP: Page.addScriptToEvaluateOnNewDocument, then Page.reload
  window.__TAURI_INTERNALS__ = {
    invoke: (cmd, args) => Promise.resolve({ list_feeds: [], list_entries: [], … }[cmd] ?? null),
    transformCallback: (cb) => cb,
  };
  ```

- Do not build on `tauri::test` (`mock_builder`/`get_ipc_response`) yet. It needs
  `tauri = { features = ["test"] }` in `[dev-dependencies]`, and every command
  and helper taking `tauri::AppHandle` must become generic over `R: Runtime`
  first (`AppHandle` defaults to `Wry`, whose `CommandArg` does not apply to
  `MockRuntime`) — that part works. But even a bare
  `mock_builder().build(mock_context(noop_assets()))` makes the test binary fail
  to load with `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` on Windows, before any
  test runs, and it survives `cargo clean -p my-timeline`. Unexplained; revisit
  before trusting command-level tests.

### Builds

```bash
npm run check                  # vue-tsc
npm test                       # vitest
cd src-tauri && cargo test     # Rust
npm run build:app              # desktop bundles (msi + nsis)
npm run android:build          # release APK
```

## When a change is implemented

Update the change's task checkboxes as work lands, and record anything the
implementation learned that changes the design (a browser behavior, a platform
limit) in that change's `design.md` — not only in the code comment.
