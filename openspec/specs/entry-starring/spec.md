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

### Requirement: Starred view always shows all starred entries

The system SHALL list every starred entry — read or unread — whenever the Starred view is selected, and SHALL NOT offer the unread filter while the Starred view is selected. The unread filter state used by "All entries" SHALL be unaffected by visiting the Starred view.

#### Scenario: Selecting Starred from the unread list

- **WHEN** the user is viewing the Unread tab of "All entries" and selects the Starred view
- **THEN** all starred entries from any feed are listed newest first with the All tab active

#### Scenario: Unread filter is not offered in the Starred view

- **WHEN** the Starred view is selected
- **THEN** no Unread tab is displayed, so the starred list cannot be filtered to unread entries

#### Scenario: Returning to All entries keeps its filter

- **WHEN** the user was on the Unread tab of "All entries", visits the Starred view, and then reselects "All entries"
- **THEN** the list shows only unread entries again

#### Scenario: Unread filter still works on All entries

- **WHEN** the user selects "All entries" and then the Unread tab
- **THEN** only unread entries are listed, newest first

### Requirement: Sidebar shows the starred entry count

The system SHALL show a count badge on the "Starred" row in the sidebar containing the total number of starred entries, counting read and unread entries alike. The badge SHALL be hidden when no entry is starred and SHALL stay current as entries are starred, unstarred, or removed.

#### Scenario: Starred count is displayed

- **WHEN** three entries are starred and the sidebar is displayed
- **THEN** the Starred row shows a badge with the number 3

#### Scenario: Counting read starred entries

- **WHEN** a starred entry is marked read
- **THEN** the Starred row's badge count does not change

#### Scenario: Badge disappears when nothing is starred

- **WHEN** the user unstars the last starred entry
- **THEN** no badge is shown on the Starred row

#### Scenario: Count drops when a starred entry's feed is removed

- **WHEN** the user removes a subscription that contained starred entries
- **THEN** the Starred row's badge decreases by the number of starred entries that were removed
