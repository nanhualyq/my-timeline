## REMOVED Requirements

### Requirement: A subscription's whole backlog can be marked read from the sidebar

**Reason**: The requirement stated that the "All entries" row offers no batch action, and one of its scenarios asserted that absence. The library-wide batch read now exists, so the requirement as written is no longer true: its scope grows from a subscription or folder to the whole library.

**Migration**: Replaced by "Batch read is offered per subscription and for the whole library", which keeps the feed and folder actions unchanged, adds the "All entries" action, and keeps the Starred row without one.

## ADDED Requirements

### Requirement: Batch read is offered per subscription and for the whole library

A feed's and a folder's sidebar row menu SHALL offer an action that marks every unread entry of that subscription or folder as read, including entries never loaded into the list. The "All entries" row's menu SHALL offer an equivalent action covering every unread entry in the library, whichever feed or folder it belongs to. Both actions SHALL be available regardless of which view or selection the list currently shows. The "Starred" row SHALL offer no batch action, and an action SHALL NOT be offered when the range it covers holds no unread entries.

#### Scenario: Marking a whole feed read

- **WHEN** a feed holds 300 unread entries of which 50 are rendered and the user activates "mark all as read" from the feed's sidebar row menu
- **THEN** all 300 become read

#### Scenario: Marking a folder read from another view

- **WHEN** the list is showing the Starred view and the user activates "mark all as read" from a folder's sidebar row menu
- **THEN** every unread entry of that folder's feeds becomes read and the Starred list's starring is unaffected

#### Scenario: Marking every entry in the library read

- **WHEN** the library holds 12,438 unread entries spread over several feeds and folders and the user activates "mark all as read" from the "All entries" row's menu
- **THEN** every stored entry becomes read, including entries never loaded into the list

#### Scenario: The global action ignores the current view

- **WHEN** the list is showing the unread view of a single feed and the user activates "mark all as read" from the "All entries" row's menu
- **THEN** every unread entry in the library becomes read, not only the selected feed's entries

#### Scenario: Starred row offers no batch action

- **WHEN** the user inspects the "Starred" sidebar row
- **THEN** it offers no batch read action

#### Scenario: Nothing unread to mark

- **WHEN** a feed, a folder, or the library has no unread entries
- **THEN** the corresponding row menu does not offer its batch action
