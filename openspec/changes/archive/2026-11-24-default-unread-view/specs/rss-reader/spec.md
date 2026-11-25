## MODIFIED Requirements

### Requirement: User can browse and filter entries

The system SHALL list stored entries newest first and SHALL offer an all-entries filter and an unread-only filter for every feed, every folder, and the global "All entries" list. Every one of those lists SHALL open on the unread filter, and the filter SHALL apply to the current selection rather than to the global list only. The Starred view SHALL list every starred entry regardless of read state and SHALL NOT offer an unread filter. Selecting an entry SHALL display its content in the reading pane.

#### Scenario: Viewing unread entries

- **WHEN** the user selects the unread view
- **THEN** only entries not yet marked read are listed, newest first

#### Scenario: Viewing a single feed's entries

- **WHEN** the user selects a feed in the sidebar
- **THEN** only that feed's entries are listed, filtered to unread because that is the default

#### Scenario: Opening a list defaults to unread

- **WHEN** the application opens, or the user selects a feed or a folder for the first time
- **THEN** the list shows unread entries only

#### Scenario: Choosing the all-entries filter inside a feed

- **WHEN** the user selects a feed and then the All tab
- **THEN** that feed's read and unread entries are listed, and the sidebar selection stays on that feed

#### Scenario: The filter survives a change of selection

- **WHEN** the user switches a feed's list to the All tab and then selects another feed
- **THEN** the newly selected feed lists read and unread entries too, because the chosen filter carries over

#### Scenario: Starred offers no filter

- **WHEN** the Starred view is selected
- **THEN** every starred entry is listed, read or unread, and no unread filter is offered
