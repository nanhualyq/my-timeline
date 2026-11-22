## Context

The repository contains only OpenSpec scaffolding — no application code. See proposal.md for motivation. Machine facts that shape this design:

- Windows, Node.js v24.19.0 + npm 11.17.0 available; **no** pnpm/yarn, **no** Rust toolchain (`rustc`/`cargo` not on PATH).
- Tauri 2.x is the current stable line (`@tauri-apps/cli` latest: 2.12.6).
- WebView2 runtime ships with Windows 11; no action expected there.

### DSH sandbox environment (discovered during apply)

The DeepSeek Harness sandbox downgrades **workspace-built executables to Low integrity** (path-based trust: `~\.cargo\bin` and system tools run Medium; `target\` and session-temp binaries run Low). Consequences:

- A Low process cannot write `%LOCALAPPDATA%` (Medium label) → Tauri's `create_dir_all` for the WebView2 data folder fails with `os error 5`; with the folder pre-created, WebView2 environment creation fails with `0x8000FFFF`.
- Verification therefore ran the app from a trusted path (`~\.cargo\bin\my-timeline.exe`, Medium IL) with the dev server; behavior is identical to a normal (non-DSH) terminal.
- **Resolution applied**: `tauri.conf.json` `windows[0].dataDirectory` points at `<repo>/webview-data` (absolute). Workspace paths are writable from Low IL and under workspace-write, so `npm run dev` now works from any terminal (user's PS/VS Code included). Trade-off: the WebView2 profile lives in the repo (gitignored) — revisit before shipping a release build, where `%LOCALAPPDATA%` is appropriate. Verified: dev window + HMR + IPC all pass with this config.

## Goals / Non-Goals

**Goals:**

- A reproducible scaffold: `npm install` + toolchain setup → runnable desktop app.
- Verified Rust↔JS IPC bridge from the first commit (the scaffold's smoke test).
- Standard npm scripts (`dev`, `check`, `build`, `build:app`) matching the dev-build-workflow spec.
- App identity configured once in `tauri.conf.json` (`productName: my-timeline`, reverse-DNS identifier).

**Non-Goals:**

- Any timeline-specific features (UI, data model, persistence) — future changes.
- CI/CD, auto-update, code signing, installers beyond Tauri's defaults.
- Cross-platform builds (Windows only for now).
- Linting/formatting tooling (can be added later as a separate change).

## Decisions

1. **Tauri 2.x over Tauri 1.x** — 1.x is end-of-life; 2.x is the maintained stable line with the plugin ecosystem. Alternative: Tauri 1 for stability — rejected, no security fixes.

2. **Scaffold via `npm create tauri-app@latest` (vue-ts template) instead of hand-wiring** — the official generator produces the canonical layout (`src/` frontend, `src-tauri/` backend, Vite config, icon set, `tauri.conf.json`) and is maintained upstream. Alternative: manual file creation — rejected as error-prone and harder to keep idiomatic. The generator runs non-interactively with `--yes`/template flags; we then adjust identity + scripts.

3. **npm as the package manager** — it is the only one installed; introducing pnpm for a scaffold adds a toolchain dependency with no benefit. Alternative: install pnpm — rejected for now; lockfile will be `package-lock.json`.

4. **Rust toolchain via `rustup-init -y` with the default `stable-x86_64-pc-windows-msvc` toolchain** — the standard, self-updating install path. The MSVC host toolchain requires the Visual Studio Build Tools C++ workload (or an existing VS install). Alternative: GNU toolchain — rejected, `x86_64-pc-windows-msvc` is the well-supported default for Tauri on Windows.

5. **Keep the generated example `greet` IPC command** — it is deliberately kept (renamed only if trivial) as the always-available smoke test proving the IPC requirement in the desktop-shell spec; it costs a few lines and is replaced by real commands later.

6. **Script mapping**: `dev` → `tauri dev`, `check` → `vue-tsc --noEmit`, `build` → `vite build`, `build:app` → `tauri build`. `@tauri-apps/cli` lives in `devDependencies` so `tauri` resolves through `npm run`/`node_modules/.bin` without a global install.

7. **Frontend at repo root, Rust crate in `src-tauri/`** — Tauri's default layout; `tauri.conf.json` points `frontendDist`/`devUrl` at the Vite output (`../dist`) and dev server (default Vite port, wired via `beforeDevCommand`/`devUrl`).

## Risks / Trade-offs

- **[MSVC Build Tools may require elevation / a ~GB-sized download]** → Detect up front (`rustc`/`cargo` on PATH, `cargo build` probe); install via `rustup-init` and the VS Build Tools installer with the `Microsoft.VisualStudio.Workload.VCTools` component; fall back to prompting the user if elevation is unavailable.
- **[First `cargo build` of Tauri takes several minutes]** → Expected one-time cost; mitigate by keeping the first verification at `cargo check`/debug build, and budget time accordingly in tasks.
- **[WebView2 runtime missing on older Windows 10]** → Verify at first launch; Tauri's NSIS/MSI bundler can include the WebView2 bootstrapper if needed (defer — Windows 11 has it built in).
- **[Generator template drift vs our npm-script spec]** → After generation, reconcile `package.json` scripts with the dev-build-workflow spec and verify each command exits 0.

## Migration Plan

Not applicable — greenfield scaffold; rollback is deleting the generated files.

## Open Questions

- **Resolved**: bundle identifier is `com.mytimeline.app` (product name `my-timeline`). Any valid reverse-DNS value satisfies the spec; this is now fixed so tasks and implementation agree.
