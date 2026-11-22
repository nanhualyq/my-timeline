# feed-folders Specification

## Purpose

Lets users group their subscriptions into named, ordered folders so a large feed list stays navigable, and browse entries across all feeds of a folder.

## Requirements

### Requirement: Feeds belong to at most one folder

The system SHALL assign each feed to zero or one folder; a feed with no folder SHALL be treated as uncategorized. Folder assignment SHALL persist across restarts.

#### Scenario: Assigning a feed to a folder

- **WHEN** the user moves a feed into a folder
- **THEN** the feed appears nested under that folder in the sidebar and the assignment survives a restart

#### Scenario: Moving a feed to another folder

- **WHEN** the user moves a feed from one folder to another
- **THEN** the feed no longer appears under the previous folder and appears under the new one

#### Scenario: A feed with no folder

- **WHEN** a feed has no folder assignment
- **THEN** it is listed in the sidebar under an uncategorized grouping rather than disappearing from the sidebar

### Requirement: Sidebar presents folders as a tree

The system SHALL present folders in the sidebar between the global views (all entries, starred) and their feeds, with feeds nested under their folder and unread counts shown per feed. The system SHALL show unread counts aggregated per folder.

#### Scenario: Sidebar structure

- **WHEN** the sidebar is displayed and at least one folder with feeds exists
- **THEN** each folder shows its name, its aggregated unread count, and its nested feeds beneath it

#### Scenario: Browsing a folder's entries

- **WHEN** the user selects a folder in the sidebar
- **THEN** the entry list shows the entries of all feeds in that folder, newest first

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

### Requirement: User can create and rename folders

The system SHALL let the user create a folder with a name and rename an existing folder. Creating or renaming SHALL fail visibly when the name is empty.

#### Scenario: Creating a folder

- **WHEN** the user creates a folder named "Tech"
- **THEN** an empty folder named "Tech" appears in the sidebar

#### Scenario: Renaming a folder

- **WHEN** the user renames "Tech" to "Technology"
- **THEN** all feeds previously under "Tech" remain under the renamed folder

### Requirement: Folders are ordered by name

The system SHALL display folders ordered by their names using default string comparison (case-sensitive, code-unit order), and SHALL NOT provide any way for the user to change folder order.

#### Scenario: Folders appear in name order regardless of creation order

- **WHEN** the user creates a folder named "Tech" and then a folder named "Books"
- **THEN** the sidebar lists "Books" before "Tech"

#### Scenario: Default string comparison is case-sensitive

- **WHEN** folders named "apple" and "Banana" exist
- **THEN** the sidebar lists "Banana" before "apple", because uppercase letters sort before lowercase ones under default string comparison

#### Scenario: Renaming changes a folder's position in the list

- **WHEN** the user renames a folder so its name sorts earlier than before
- **THEN** the sidebar updates and the folder appears at the position implied by its new name

#### Scenario: No reorder controls are shown

- **WHEN** the sidebar displays a folder row
- **THEN** the row offers no control for moving the folder up or down

### Requirement: Deleting a folder reassigns its feeds

The system SHALL let the user delete a folder; deleting a folder SHALL move its feeds to uncategorized and SHALL NOT delete the feeds or their entries.

#### Scenario: Deleting a folder with feeds

- **WHEN** the user deletes a folder containing two feeds
- **THEN** the folder disappears, its two feeds appear under the uncategorized grouping, and all their entries remain stored

### Requirement: Removing a feed leaves its folder intact

Removing a feed SHALL NOT modify its folder; a folder whose last feed is removed SHALL remain in the sidebar.

#### Scenario: Empty folder after feed removal

- **WHEN** the user removes the only feed in a folder
- **THEN** the folder is still listed in the sidebar with a zero unread count
