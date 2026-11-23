## MODIFIED Requirements

### Requirement: Global starred view

The system SHALL provide a global "Starred" view in the sidebar that makes all
starred entries reachable newest first, regardless of which feed or folder they
belong to. Selecting a feed or folder SHALL switch away from the starred view;
the starred view SHALL NOT be combined with a feed or folder selection.

#### Scenario: Viewing starred entries

- **WHEN** the user selects the Starred view
- **THEN** starred entries from any feed are listed newest first, and the
  remaining starred entries are reachable by loading further batches

#### Scenario: Starred view is global

- **WHEN** the user has starred entries across two different feeds and selects
  the Starred view
- **THEN** entries from both feeds appear in one list

#### Scenario: Selecting a feed leaves the starred view

- **WHEN** the user is in the Starred view and selects a feed in the sidebar
- **THEN** the entry list shows only that feed's entries

### Requirement: Starred view always shows all starred entries

The system SHALL make every starred entry — read or unread — reachable whenever
the Starred view is selected, and SHALL NOT offer the unread filter while the
Starred view is selected. The unread filter state used by "All entries" SHALL
be unaffected by visiting the Starred view.

#### Scenario: Selecting Starred from the unread list

- **WHEN** the user is viewing the Unread tab of "All entries" and selects the
  Starred view
- **THEN** starred entries from any feed are listed newest first with the All
  tab active, and every starred entry remains reachable by loading further
  batches

#### Scenario: Unread filter is not offered in the Starred view

- **WHEN** the Starred view is selected
- **THEN** no Unread tab is displayed, so the starred list cannot be filtered
  to unread entries

#### Scenario: Returning to All entries keeps its filter

- **WHEN** the user was on the Unread tab of "All entries", visits the Starred
  view, and then reselects "All entries"
- **THEN** the list shows only unread entries again

#### Scenario: Unread filter still works on All entries

- **WHEN** the user selects "All entries" and then the Unread tab
- **THEN** only unread entries are listed, newest first

#### Scenario: Scrolling through starred entries

- **WHEN** the user has starred more entries than fit on one batch and reaches
  the end of the loaded rows
- **THEN** further starred entries are loaded and appended, read and unread
  alike, with no unread filter applied
