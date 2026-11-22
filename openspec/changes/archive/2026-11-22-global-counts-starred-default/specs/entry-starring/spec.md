## REMOVED Requirements

### Requirement: Starred view respects the unread filter
**Reason**: The Starred view now always shows every starred entry, so there is no unread filter left to apply there.
**Migration**: No data migration. Users who previously filtered the Starred view to unread-only entries will see those entries again in the full starred list; each entry's read/unread state and the unread filter on "All entries" are unchanged.

## ADDED Requirements

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
