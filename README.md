# Tauri + Vue + TypeScript

This template should help get you started developing with Vue 3 and TypeScript in Vite. The template uses Vue 3 `<script setup>` SFCs, check out the [script setup docs](https://v3.vuejs.org/api/sfc-script-setup.html#sfc-script-setup) to learn more.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Desktop

```bash
npm install
npm run dev        # hot-reloading Tauri window
npm run check      # TypeScript type-check
npm test           # Vitest suite
npm run build:app  # packaged Windows application
```

### Verifying UI behavior

Do not verify by screenshot — it only proves a window rendered. The Android build
can be driven at the DOM level over the DevTools protocol (emulator + `adb` +
`Runtime.evaluate` / `Input.dispatchTouchEvent`), which exercises real gestures;
the desktop build is driven by hand through WebView2's DevTools. `AGENTS.md` has
the exact commands, including why `--remote-debugging-port` does not work for the
desktop webview.

Desktop builds need only Node.js, npm, and a Rust toolchain.

## Android

The Android build shares the frontend and the Rust backend with the desktop
build; only the toolchain and the packaging step differ. Each installation
keeps its own local library.

### Prerequisites

Install all of the following before running any Android command:

1. **JDK 17 or newer.** Tauri's Android build runs Gradle, which requires a JDK
   the command line can see. Check with `java -version`; if it is missing,
   install a JDK 17+ distribution and set `JAVA_HOME`.
2. **Android SDK** — the command-line tools plus the platform and build tools
   for API 34 or newer. Set `ANDROID_HOME` (or `ANDROID_SDK_ROOT`) to the SDK
   directory, and make sure `platform-tools` is on `PATH` for `adb`.
3. **Android NDK** — installed through the SDK manager. Set `NDK_HOME` to the
   installed NDK directory, for example
   `$ANDROID_HOME/ndk/26.1.10909125`.
4. **Rust Android targets** — the target for a physical device, and the
   emulator target if you use one:

   ```bash
   rustup target add aarch64-linux-android
   rustup target add x86_64-linux-android   # emulator only
   ```

5. **A device or emulator.** For a physical device, enable developer options
   and USB debugging, then confirm it is visible with `adb devices`.

If a prerequisite is missing, the Android commands fail with a message naming
it; the message comes from the underlying tool, so read the first line rather
than the wrapped output.

### Commands

```bash
npm run android:init   # once per checkout: generates src-tauri/gen/android
npm run android:dev    # build, install, and run with hot reload on a device
npm run android:build  # release APK under src-tauri/gen/android/app/build/outputs/apk
```

`npm run android:init` only needs to run once per checkout; the generated
project under `src-tauri/gen/android` is committed, because it carries the
manifest, the launcher icons, and `MainActivity`.

### Release signing

The release keystore is committed on purpose, so a fresh checkout produces a
signed APK with no extra setup:

- `src-tauri/gen/android/keystore/mytimeline-release.jks` — key alias
  `mytimeline`, store and key password `Timeline@2025`
- `src-tauri/gen/android/keystore.properties` — the values
  `app/build.gradle.kts` reads

```bash
npm run android:build
# -> src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

`app/build.gradle.kts` binds the release signing config only when
`keystore.properties` is present; without it the build still succeeds but
leaves the APK unsigned. Check which key a build uses with:

```bash
cd src-tauri/gen/android && ./gradlew :app:signingReport -q | grep -A4 Release
```

Because the repository is public, this private key is public as well. Nothing
about a store listing can be hijacked — the app is sideloaded — but anyone can
sign an APK with the same application id, and it would install as an update
over an existing installation and inherit its data, including the WebDAV
credentials stored in it. Only install `com.mytimeline.app` builds that came
from this repository.

Replacing the key later does not break anything, but it cannot upgrade over the
current installation: the old app has to be uninstalled first, which drops its
local library unless it was exported.

The Android commands require the Android toolchain. The desktop commands above
do not, so a machine without the SDK can still build and test the desktop app.
