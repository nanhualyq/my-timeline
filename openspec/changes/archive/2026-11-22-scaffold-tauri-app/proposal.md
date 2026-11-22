## Why

The project repository is empty apart from OpenSpec scaffolding, so there is no runnable application to build on. To develop the timeline desktop app, the project needs a working Tauri 2 foundation with a Vue 3 + TypeScript frontend. This machine currently has no Rust toolchain, so bootstrapping the toolchain is a prerequisite for any Tauri work.

## What Changes

- Install the Rust toolchain on the developer machine (rustup + MSVC Build Tools on Windows) as a prerequisite for compiling the Tauri backend.
- Scaffold a Tauri 2 application with a Vite + Vue 3 + TypeScript frontend in this repository.
- Configure the app identity (bundle identifier, product name, main window properties) for the `my-timeline` desktop app.
- Establish npm scripts for the development workflow: hot-reload dev run, type checking, and a production build that produces a runnable Windows artifact.
- Keep a minimal example Rust command wired to the frontend so the Rust↔JavaScript IPC bridge is verifiably working from day one.

## Capabilities

### New Capabilities

- `desktop-shell`: The desktop application boots a native window that loads the Vue frontend, with configured window properties, and a round-trip IPC call between the webview and the Rust backend succeeds.
- `dev-build-workflow`: Standard npm commands run the app in development with hot reload, type-check the codebase, and produce a production build artifact.

### Modified Capabilities

(none — no existing specs in `openspec/specs/`)

## Impact

- **New project files**: `package.json`, `vite.config.ts`, `tsconfig*.json`, `index.html`, `src/` (Vue app), `src-tauri/` (Rust crate: `Cargo.toml`, `tauri.conf.json`, `src/main.rs`, `src/lib.rs`, icons).
- **New dependencies**: `vue`, `@tauri-apps/api`, `@tauri-apps/cli`, `typescript`, `vite`, plus Rust crates `tauri`, `tauri-build`.
- **System prerequisites**: Rust toolchain (`rustup`, `rustc`, `cargo`) and MSVC Build Tools + Windows SDK (not currently installed); WebView2 runtime (preinstalled on Windows 11).
- **No existing code affected** — greenfield repository.
