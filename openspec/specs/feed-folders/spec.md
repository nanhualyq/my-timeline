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

### Requirement: User can create and rename folders

The system SHALL let the user create a folder with a name and rename an existing folder. Creating or renaming SHALL fail visibly when the name is empty.

#### Scenario: Creating a folder

- **WHEN** the user creates a folder named "Tech"
- **THEN** an empty folder named "Tech" appears in the sidebar

#### Scenario: Renaming a folder

- **WHEN** the user renames "Tech" to "Technology"
- **THEN** all feeds previously under "Tech" remain under the renamed folder

### Requirement: User can reorder folders

The system SHALL let the user change the display order of folders, and the chosen order SHALL persist across restarts.

#### Scenario: Reordering folders

- **WHEN** the user moves folder "B" above folder "A"
- **THEN** the sidebar lists "B" before "A" and the order is the same after a restart

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
