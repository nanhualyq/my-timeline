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

The Android commands require the Android toolchain. The desktop commands above
do not, so a machine without the SDK can still build and test the desktop app.
