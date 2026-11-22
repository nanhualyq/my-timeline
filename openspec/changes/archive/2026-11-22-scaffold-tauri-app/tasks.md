## 1. Toolchain bootstrap

- [x] 1.1 Install the Rust stable toolchain via `rustup-init` with the default `x86_64-pc-windows-msvc` host, then verify `rustc --version` and `cargo --version` both report a stable release
- [x] 1.2 Ensure the MSVC C++ build tools (Visual Studio Build Tools with the `Microsoft.VisualStudio.Workload.VCTools` workload, or an existing Visual Studio installation) are present, then verify by compiling and linking a throwaway probe crate in the temp directory with `cargo build` exiting 0
- [x] 1.3 Verify the WebView2 runtime is available on this machine (Windows 11 ships it built in; confirm the `WebView2` registry key exists), and record the result — the first app launch in task 4.1 is the end-to-end proof

## 2. Application scaffold

- [x] 2.1 Scaffold the Tauri 2 + Vue 3 + TypeScript app into the repository root using `npm create tauri-app@latest` non-interactively (template `vue-ts`, accept defaults; use its `--help` for exact flags), then verify `package.json`, `vite.config.ts`, `index.html`, `tsconfig*.json`, `src/`, and `src-tauri/` (including `tauri.conf.json`, `src/main.rs`, `src/lib.rs`, `Cargo.toml`) exist alongside the untouched `openspec/` and `.agents/` directories
- [x] 2.2 Configure app identity in `src-tauri/tauri.conf.json` — `productName` `my-timeline`, identifier `com.mytimeline.app`, window title `my-timeline` — then verify the file parses as valid JSON and all three values match
- [x] 2.3 Verify the example `greet` IPC command survives scaffolding on both sides: the Rust `#[tauri::command]` handler is registered in the invoke handler and the frontend calls it (runtime proof happens in task 4.2)
- [x] 2.4 Run `npm install` and verify it exits 0 with a `package-lock.json` created at the repo root
- [x] 2.5 Verify `.gitignore` covers `node_modules/`, `dist/`, and `src-tauri/target/`

## 3. npm scripts

- [x] 3.1 Wire the scripts in `package.json` per design decision 6 — `dev` → `tauri dev`, `check` → `vue-tsc --noEmit`, `build` → `vite build`, `build:app` → `tauri build` — and verify `npm run check` exits 0 (fix any template type errors it reports)
- [x] 3.2 Verify `npm run build` exits 0 and produces a `dist/` directory containing static frontend assets (frontend-bundle scenario of the dev-build-workflow spec)

## 4. Runtime verification (dev-build-workflow + desktop-shell specs)

- [x] 4.1 Run `npm run dev` and verify a native window titled `my-timeline` opens showing the Vue frontend with no blank screen or resource-load errors, and that editing a frontend source file hot-reloads the window without restarting the Rust process
- [x] 4.2 In the running app, trigger the `greet` IPC call and verify the returned greeting is rendered with no console errors (desktop-shell IPC scenario)
- [x] 4.3 Run `npm run build:app` and verify it exits 0 with a `my-timeline` Windows executable under `src-tauri/target/release/bundle/`, and that launching that executable opens the window showing the built frontend (production scenarios of both specs)

## 5. Planning artifact validation

- [x] 5.1 Run `openspec validate "scaffold-tauri-app" --strict` and verify it reports the change as valid
