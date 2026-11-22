## MODIFIED Requirements

### Requirement: Rust-to-frontend IPC bridge works

The system SHALL expose at least one Rust command that the frontend can invoke, and the frontend SHALL render the value returned by the backend.

#### Scenario: Round-trip IPC call succeeds

- **WHEN** the frontend invokes a reader Rust command (for example, listing feeds)
- **THEN** the Rust backend returns data derived from the application state and the frontend renders it without console errors
