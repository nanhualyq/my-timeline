## MODIFIED Requirements

### Requirement: Sidebar presents folders as a tree

The system SHALL present folders in the sidebar between the global views (all entries, starred) and their feeds, with feeds nested under their folder and unread counts shown per feed. The system SHALL show unread counts aggregated per folder. A folder's nested feeds SHALL be displayed beneath it only while that folder is expanded.

#### Scenario: Sidebar structure

- **WHEN** the sidebar is displayed and at least one expanded folder with feeds exists
- **THEN** each folder shows its name and its aggregated unread count, and each expanded folder additionally shows its nested feeds beneath it

#### Scenario: Browsing a folder's entries

- **WHEN** the user selects a folder in the sidebar
- **THEN** the entry list shows the entries of all feeds in that folder, newest first

#### Scenario: Collapsed folder keeps its row visible

- **WHEN** the sidebar is displayed and a folder with feeds is collapsed
- **THEN** the folder's name and aggregated unread count remain visible while its feed rows are hidden

### Requirement: Folder row actions live in a menu

The system SHALL expose a folder row's actions — refresh folder, rename, and delete — through a single dropdown menu control on that row, and the row SHALL display only the folder's expand/collapse toggle, its name, its aggregated unread count, and that menu control.

#### Scenario: Folder menu contents

- **WHEN** the user opens a folder row's menu
- **THEN** it offers "Refresh folder", "Rename", and "Delete"

#### Scenario: Row shows only name and count

- **WHEN** a folder row is rendered
- **THEN** it shows the expand/collapse toggle, the folder name, its unread badge, and the menu control, with no other inline action buttons beside the name

## ADDED Requirements

### Requirement: Folders expand and collapse

The system SHALL let the user expand and collapse a folder with a toggle control placed at the front of the folder row, before the folder name. All folders SHALL be collapsed when the app starts, and expansion state SHALL NOT persist across restarts. Selecting a folder SHALL NOT change its expansion state.

#### Scenario: Expanding a folder

- **WHEN** the user activates the toggle on a collapsed folder
- **THEN** that folder's nested feed rows appear beneath it, while other folders' expansion states are unaffected

#### Scenario: Collapsing a folder

- **WHEN** the user activates the toggle on an expanded folder
- **THEN** that folder's nested feed rows are hidden while its name and aggregated unread count stay visible

#### Scenario: Every launch starts with all folders collapsed

- **WHEN** the app starts after a session in which folders were expanded
- **THEN** all folders are collapsed

#### Scenario: Selecting a folder does not toggle expansion

- **WHEN** the user clicks the name of a collapsed folder to browse its entries
- **THEN** the folder stays collapsed and the entry list still shows the folder's entries
