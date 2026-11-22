## ADDED Requirements

### Requirement: Folder row actions live in a menu

The system SHALL expose a folder row's actions — refresh folder, rename, and delete — through a single dropdown menu control on that row, and the row SHALL display only the folder's name, its aggregated unread count, and that menu control.

#### Scenario: Folder menu contents

- **WHEN** the user opens a folder row's menu
- **THEN** it offers "Refresh folder", "Rename", and "Delete"

#### Scenario: Row shows only name and count

- **WHEN** a folder row is rendered
- **THEN** it shows the folder name, its unread badge, and the menu control, with no inline action buttons beside the name

### Requirement: User can refresh a folder

The system SHALL let the user refresh every feed assigned to a folder in one action, fetching those feeds with the same bounded concurrency and per-feed failure isolation as a full refresh. When the refresh completes, the system SHALL show a single summary of how many feeds refreshed successfully and how many failed.

#### Scenario: Refreshing a folder successfully

- **WHEN** the user triggers "Refresh folder" and all of the folder's feeds respond successfully
- **THEN** each feed's entries and last-fetched timestamps are updated and the summary reports all feeds refreshed with no failures

#### Scenario: Partial failure during a folder refresh

- **WHEN** a folder refresh completes and some of the folder's feeds failed
- **THEN** the summary reports both the refreshed and failed counts, the failing feeds show their error indicators, and the successful feeds' entries are stored
