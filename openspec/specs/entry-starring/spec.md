# entry-starring Specification

## Purpose

Lets users bookmark individual entries with a star and return to them later through a dedicated global view, independent of read state and of the selected feed or folder.

## Requirements

### Requirement: User can star and unstar an entry

The system SHALL let the user toggle a star on any stored entry, SHALL persist the star across restarts, and SHALL show the current star state on the entry in the list.

#### Scenario: Starring an entry

- **WHEN** the user stars an entry
- **THEN** the entry shows as starred in the list and remains starred after a restart

#### Scenario: Unstarring an entry

- **WHEN** the user stars an already-starred entry
- **THEN** the star is removed and the entry no longer appears as starred

### Requirement: Starred state is independent of read state

Starring SHALL NOT change an entry's read state, and marking an entry read or unread SHALL NOT change its star state.

#### Scenario: Starring a read entry

- **WHEN** the user stars an entry that is already read
- **THEN** the entry stays read and becomes starred

### Requirement: Global starred view

The system SHALL provide a global "Starred" view in the sidebar that lists all starred entries newest first, regardless of which feed or folder they belong to. Selecting a feed or folder SHALL switch away from the starred view; the starred view SHALL NOT be combined with a feed or folder selection.

#### Scenario: Viewing starred entries

- **WHEN** the user selects the Starred view
- **THEN** every starred entry from any feed is listed newest first

#### Scenario: Starred view is global

- **WHEN** the user has starred entries across two different feeds and selects the Starred view
- **THEN** entries from both feeds appear in one list

#### Scenario: Selecting a feed leaves the starred view

- **WHEN** the user is in the Starred view and selects a feed in the sidebar
- **THEN** the entry list shows only that feed's entries

### Requirement: Starred view respects the unread filter

The system SHALL apply the existing all/unread filter to the starred view like any other view.

#### Scenario: Unread starred entries only

- **WHEN** the user selects the Starred view with the unread filter active
- **THEN** only entries that are both starred and unread are listed
