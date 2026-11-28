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

### Desktop: `npm run dev`, driven by hand or by WebView2 DevTools

```bash
npm run dev        # debug build, data dir com.mytimeline.app-dev (runs beside a release install)
```

The dev webview keeps WebView2's own DevTools (F12 / right-click → Inspect) for
DOM inspection and hand-driving.

**CDP over a port does not work here.** Passing `--remote-debugging-port=9222`
either through `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` or through the window
config (`npx tauri dev --config '{"app":{"windows":[{"additionalBrowserArgs":"…"}]}}'`)
makes webview creation fail outright:

```
Failed to setup app: runtime error: failed to create webview:
WebView2 error: WindowsError(Error { code: HRESULT(0x8007139F), message:
"The group or resource is not in the correct state to perform the requested operation." })
```

wry always supplies `additionalBrowserArgs`, and the port conflicts with
WebView2's own transport, so the switch is not available to this app. For
behavior, verify on Android (same frontend); on desktop verify layout and the
wide-layout-only controls by reading the DOM in the DevTools console.

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
