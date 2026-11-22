## ADDED Requirements

### Requirement: Feed row actions live in a menu

The system SHALL expose a feed row's actions — edit, refresh, and remove — through a single dropdown menu control on that row, and the row SHALL display only the feed's title, its unread badge, and that menu control.

#### Scenario: Feed menu contents

- **WHEN** the user opens a feed row's menu
- **THEN** it offers "Edit…", "Refresh", and "Remove"

#### Scenario: Row shows only title and count

- **WHEN** a feed row is rendered
- **THEN** it shows the feed title, its unread badge, and the menu control, with no inline refresh button, folder select, or remove button beside the name

### Requirement: User can edit a feed in a dialog

The system SHALL provide an edit dialog for a feed, opened from its row menu, containing a folder assignment control that moves the feed between folders or to uncategorized, a custom title input, and read-only status information (feed URL, last fetched time, last fetch error). A custom title SHALL persist across refreshes — subsequent fetches SHALL NOT overwrite it — and clearing the custom title SHALL revert the displayed title to the feed-provided one. Removal SHALL NOT be part of the dialog.

#### Scenario: Assigning a folder from the dialog

- **WHEN** the user opens a feed's edit dialog, chooses a folder, and saves
- **THEN** the feed appears nested under that folder in the sidebar

#### Scenario: Setting a custom title

- **WHEN** the user sets a custom title and later refreshes the feed
- **THEN** the sidebar and reading pane keep showing the custom title after the refresh

#### Scenario: Clearing a custom title

- **WHEN** the user clears a previously set custom title and saves
- **THEN** the displayed title reverts to the title provided by the feed

#### Scenario: Status info is read-only

- **WHEN** the user opens the edit dialog
- **THEN** the feed URL, last fetched time, and last fetch error are shown and cannot be edited there
