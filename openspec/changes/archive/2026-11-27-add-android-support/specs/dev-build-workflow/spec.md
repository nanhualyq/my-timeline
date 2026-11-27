## ADDED Requirements

### Requirement: Android development runs on a device or emulator

The project SHALL provide Android development commands: one that initializes the Android project when it is absent, and one that builds the application for Android and runs it against the same frontend development server used by desktop development, so frontend edits are reflected without a Rust rebuild.

#### Scenario: Initializing the Android project

- **WHEN** the developer runs the Android initialization command on a checkout without a generated Android project
- **THEN** the Android project is generated under the Tauri application directory and the command exits with status 0

#### Scenario: Running on a device

- **WHEN** the developer runs the Android development command with a device or emulator attached
- **THEN** the application is installed and opened on that target and loads the frontend from the development server

#### Scenario: Frontend edits hot-reload on Android

- **WHEN** a frontend source file is saved while Android development is running
- **THEN** the running application reflects the change without a full rebuild

### Requirement: Android packaging produces an installable artifact

The project SHALL provide an Android build command that produces an installable debug-signable package under the generated Android project's build output, and the command SHALL exit with a non-zero status when the build fails.

#### Scenario: Packaging for Android

- **WHEN** the developer runs the Android build command
- **THEN** an Android package file for `my-timeline` exists under the Android project's build output and the command exits with status 0

#### Scenario: A broken build reports failure

- **WHEN** the Android build command is run while the Rust or frontend code does not compile
- **THEN** the command reports the error and exits with a non-zero status

### Requirement: Android toolchain requirements are documented and detected

The project SHALL document the Android development prerequisites — the Android SDK, the Android NDK, a supported JDK, and the Android Rust targets for the device and emulator architectures — and Android build commands SHALL fail with a message naming the missing prerequisite rather than an opaque toolchain error.

#### Scenario: Missing toolchain is explained

- **WHEN** a developer runs an Android command without the Android SDK or the Android Rust targets installed
- **THEN** the command fails and names the missing prerequisite

#### Scenario: Documented prerequisites are sufficient

- **WHEN** a developer installs exactly the documented prerequisites on a clean machine
- **THEN** the Android development and build commands succeed without further environment changes

### Requirement: Desktop commands and artifacts are unchanged

The existing desktop development, type-check, and packaging commands SHALL keep their current behavior, and the desktop build SHALL continue to emit its Windows artifact. Adding Android support SHALL NOT make a desktop command require the Android toolchain.

#### Scenario: Desktop build without Android tooling

- **WHEN** the developer runs the desktop packaging command on a machine with no Android SDK installed
- **THEN** the desktop artifact is produced as before

#### Scenario: Type-check covers shared frontend code

- **WHEN** the developer runs the type-check command after the narrow-layout work
- **THEN** the shared frontend code type-checks with the same command and exit behavior as before
