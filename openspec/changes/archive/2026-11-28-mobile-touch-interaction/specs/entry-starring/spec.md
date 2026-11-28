## ADDED Requirements

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
