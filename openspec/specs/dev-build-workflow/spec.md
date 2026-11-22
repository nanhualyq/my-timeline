# dev-build-workflow Specification

## Purpose

Defines the standard developer commands for running the application with hot reload, validating the codebase, and packaging a production build.

## Requirements

### Requirement: Development mode runs with hot reload

The project SHALL provide an `npm run dev` command that starts the frontend development server and opens the Tauri window against it. Frontend source edits SHALL be reflected in the running window without restarting the Rust process.

#### Scenario: Start development mode

- **WHEN** the developer runs `npm run dev`
- **THEN** the frontend dev server starts and a Tauri window opens loading the application from it

#### Scenario: Frontend edit hot-reloads

- **WHEN** a frontend source file is saved while development mode is running
- **THEN** the open window reflects the change without a manual rebuild or restart of the Rust process

### Requirement: Type-check command reports errors

The project SHALL provide an `npm run check` command that type-checks the TypeScript codebase and exits with a non-zero status when type errors exist.

#### Scenario: Check passes on a healthy codebase

- **WHEN** the developer runs `npm run check` and the codebase has no type errors
- **THEN** the command exits with status 0

#### Scenario: Check fails on type errors

- **WHEN** the developer runs `npm run check` and the codebase contains a type error
- **THEN** the command reports the error and exits with a non-zero status

### Requirement: Production build produces a Windows artifact

The project SHALL provide an `npm run build` command that emits the static frontend bundle, and an `npm run build:app` command that packages the full desktop application into a Windows executable under the Rust target directory.

#### Scenario: Frontend bundle is emitted

- **WHEN** the developer runs `npm run build`
- **THEN** a `dist/` directory containing the static frontend assets is produced and the command exits with status 0

#### Scenario: Desktop application is packaged

- **WHEN** the developer runs `npm run build:app`
- **THEN** a Windows executable (and/or installer) for `my-timeline` exists under `src-tauri/target/release/bundle/` and the command exits with status 0
