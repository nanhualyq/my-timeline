## MODIFIED Requirements

### Requirement: Settings dialog hosts infrequent subscription-management actions

The system SHALL provide a Settings dialog containing an add-feed-by-URL input, a create-folder action, OPML import and export actions, sync-document import and export actions, and a WebDAV sync section with the connection settings, the last sync status, and a "Sync now" action. Each action SHALL retain the behavior required by its own capability (adding a feed, creating a folder, OPML import/export, sync-document import/export, WebDAV sync), and every validation error and outcome message for these actions SHALL be visible while the Settings dialog is open.

#### Scenario: Opening settings reveals the management actions

- **WHEN** the user opens the Settings dialog
- **THEN** it shows a feed URL input, a folder-name input, Import OPML / Export OPML controls, Import Sync Document / Export Sync Document controls, and a sync section with the server URL, remote path, username, password, certificate toggle, last-sync status, and a "Sync now" button

#### Scenario: Adding a feed from the Settings dialog

- **WHEN** the user enters a valid feed URL in the Settings dialog and submits it
- **THEN** a success or error message is shown inside the dialog and, on success, the new feed appears in the sidebar

#### Scenario: Import summary is reported in the Settings dialog

- **WHEN** the user imports an OPML file from the Settings dialog
- **THEN** the import summary (feeds added, feeds skipped, folders created) is shown in the dialog, and the imported feeds and folders appear in the sidebar

#### Scenario: Creating a folder from the Settings dialog

- **WHEN** the user creates a folder named "Tech" from the Settings dialog
- **THEN** an empty folder named "Tech" appears in the sidebar and the dialog reports success

#### Scenario: Sync-document import summary is reported in the Settings dialog

- **WHEN** the user imports a sync document from the Settings dialog
- **THEN** the applied-changes summary is shown in the dialog, and the imported feeds and folders appear in the sidebar

#### Scenario: Sync-document export outcome is reported in the Settings dialog

- **WHEN** the user exports a sync document from the Settings dialog and picks a destination
- **THEN** the dialog reports that the document was written

#### Scenario: A rejected sync document reports why

- **WHEN** the user picks a sync document that this build cannot read
- **THEN** the dialog shows the reason it was rejected and the sidebar is unchanged

#### Scenario: WebDAV settings are edited and saved in the dialog

- **WHEN** the user enters a server URL, remote path, username, and password and saves them
- **THEN** the dialog reports that the connection was saved and keeps the values on screen

#### Scenario: The dialog reports the last sync

- **WHEN** the user opens the Settings dialog after a sync ran
- **THEN** the sync section shows when that sync ran and what it did, or why it failed

#### Scenario: Sync now runs from the dialog

- **WHEN** the user activates "Sync now" in the dialog
- **THEN** a sync runs and the status line shows its outcome without the dialog closing or the selection changing

#### Scenario: The dialog keeps closing on Escape and overlay click

- **WHEN** the Settings dialog is open and the user presses Escape or clicks the overlay
- **THEN** the dialog closes and the current selection and data are unchanged
