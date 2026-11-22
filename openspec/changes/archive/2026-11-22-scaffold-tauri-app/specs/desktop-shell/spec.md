## Purpose

Guarantees the my-timeline desktop application starts as a native window presenting the web frontend, with a verified communication channel between the webview and the Rust backend.

## ADDED Requirements

### Requirement: Application opens a native window with the frontend

The application SHALL open a native desktop window when launched, and the window SHALL load the Vue frontend. The window SHALL present `my-timeline` as its title and SHALL use the default dimensions defined in the application configuration.

#### Scenario: Launching in development mode opens the window

- **WHEN** the developer starts the application in development mode
- **THEN** a native window opens displaying the Vue frontend with no blank-screen or resource-load errors

#### Scenario: Launching the production build opens the window

- **WHEN** the packaged production executable is run
- **THEN** a native window opens displaying the built Vue frontend with no blank-screen or resource-load errors

### Requirement: Rust-to-frontend IPC bridge works

The system SHALL expose at least one Rust command that the frontend can invoke, and the frontend SHALL render the value returned by the backend.

#### Scenario: Round-trip IPC call succeeds

- **WHEN** the frontend invokes the example Rust command with a name argument
- **THEN** the Rust backend returns a greeting containing that name and the frontend displays it without console errors

### Requirement: App identity is configured

The application SHALL be configured with the product name `my-timeline` and a reverse-DNS bundle identifier, so that built artifacts are identifiable as this application.

#### Scenario: Built artifact carries the configured identity

- **WHEN** a production build is produced
- **THEN** the resulting executable is named for the product `my-timeline` and the bundle metadata contains the configured reverse-DNS identifier
