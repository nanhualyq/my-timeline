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

The system SHALL provide a global "Starred" view in the sidebar that makes all starred entries reachable newest first, regardless of which feed or folder they belong to. Selecting a feed or folder SHALL switch away from the starred view; the starred view SHALL NOT be combined with a feed or folder selection.

#### Scenario: Viewing starred entries

- **WHEN** the user selects the Starred view
- **THEN** starred entries from any feed are listed newest first, and the remaining starred entries are reachable by loading further batches

#### Scenario: Starred view is global

- **WHEN** the user has starred entries across two different feeds and selects the Starred view
- **THEN** entries from both feeds appear in one list

#### Scenario: Selecting a feed leaves the starred view

- **WHEN** the user is in the Starred view and selects a feed in the sidebar
- **THEN** the entry list shows only that feed's entries

### Requirement: Starred view always shows all starred entries

The system SHALL make every starred entry — read or unread — reachable whenever the Starred view is selected, and SHALL NOT offer the unread filter while the Starred view is selected. The unread filter state used by "All entries" SHALL be unaffected by visiting the Starred view.

#### Scenario: Selecting Starred from the unread list

- **WHEN** the user is viewing the Unread tab of "All entries" and selects the Starred view
- **THEN** starred entries from any feed are listed newest first with the All tab active, and every starred entry remains reachable by loading further batches

#### Scenario: Unread filter is not offered in the Starred view

- **WHEN** the Starred view is selected
- **THEN** no Unread tab is displayed, so the starred list cannot be filtered to unread entries

#### Scenario: Returning to All entries keeps its filter

- **WHEN** the user was on the Unread tab of "All entries", visits the Starred view, and then reselects "All entries"
- **THEN** the list shows only unread entries again

#### Scenario: Unread filter still works on All entries

- **WHEN** the user selects "All entries" and then the Unread tab
- **THEN** only unread entries are listed, newest first

#### Scenario: Scrolling through starred entries

- **WHEN** the user has starred more entries than fit on one batch and reaches the end of the loaded rows
- **THEN** further starred entries are loaded and appended, read and unread alike, with no unread filter applied

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

### Requirement: The reading pane shows and toggles the open entry's star

The reading pane SHALL show whether the open entry is starred and SHALL offer a control that toggles it. The toggle SHALL present a touch-sized target on a touch layout and SHALL convey its state and purpose through an accessible name rather than through its glyph alone. Toggling it SHALL update the entry's row in the list and the sidebar's starred count without reopening the entry, and SHALL NOT change the entry's read state.

#### Scenario: Starring from the reading pane

- **WHEN** the reader activates the star control while reading an unstarred entry
- **THEN** the entry becomes starred, the control shows the starred state, and the entry's row in the list reflects it

#### Scenario: Unstarring from the reading pane

- **WHEN** the reader activates the star control while reading a starred entry
- **THEN** the entry is unstarred and the sidebar's starred count decreases

#### Scenario: The star is reachable without returning to the list

- **WHEN** the reader is inside an article on a touch layout and wants to star it
- **THEN** the star control is available in the reading pane without leaving the reading screen

#### Scenario: Starring does not change the read state

- **WHEN** the reader stars a read entry from the reading pane
- **THEN** the entry stays read and the reading pane still offers to mark it unread

#### Scenario: An entry reachable only by reading

- **WHEN** the reader has opened an entry whose row is no longer among the loaded rows
- **THEN** the reading pane still shows its star state and still toggles it
