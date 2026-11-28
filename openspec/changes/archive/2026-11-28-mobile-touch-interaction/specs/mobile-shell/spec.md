## ADDED Requirements

### Requirement: Touch affordances follow the primary pointer, not the width

The application SHALL apply its touch affordances — row gestures, the row long-press menu, touch-sized targets, and the absence of hover-revealed controls — whenever the viewport is too narrow for three columns or the primary pointer is coarse. The choice between one pane and three columns SHALL continue to follow the viewport's width and SHALL NOT change because the primary pointer is coarse.

#### Scenario: A landscape phone

- **WHEN** the application runs on a device whose primary pointer is coarse and whose wide but short viewport is too narrow to hold three usable columns
- **THEN** the reader gets the one-pane layout with the touch affordances

#### Scenario: A tablet in landscape

- **WHEN** the application runs on a coarse-pointer device whose viewport holds three columns
- **THEN** the three columns are shown, and the rows within the entry list offer the touch affordances rather than hover-revealed controls

#### Scenario: A desktop window

- **WHEN** the application runs in a desktop window with a fine pointer
- **THEN** rows keep their hover-revealed controls and no row gesture acts on them

## MODIFIED Requirements

### Requirement: Wide viewports keep the three-column layout

When the viewport is wide enough to hold the sidebar, the entry list, and the reading pane side by side, the application SHALL render them as the three simultaneous columns it renders on desktop, with the same content and ordering. Narrow-viewport behavior SHALL NOT alter the wide layout, and the primary pointer's coarseness SHALL NOT change how many columns the wide layout shows.

#### Scenario: A wide window on desktop

- **WHEN** the application runs in a window wide enough for three columns
- **THEN** the sidebar, entry list, and reading pane are visible at the same time as before this change

#### Scenario: A wide viewport on a tablet

- **WHEN** the application runs on an Android device whose width holds three columns
- **THEN** the same three-column layout is presented

### Requirement: Every desktop action is reachable by touch

On a touch layout, every action available on desktop SHALL be reachable without a hover state and without relying on a tooltip, through a gesture, a long-press menu, or a visible control. Row-level actions that desktop reveals on hover — including starring an entry and marking the entries above it read — SHALL be reachable by gesture and by the row's long-press menu, and no action SHALL be available only while a pointer rests on a row.

#### Scenario: Marking entries above read by touch

- **WHEN** a touch layout shows a row with unread entries above it
- **THEN** the action that marks those entries read is reachable by dragging the row rightward and through the row's long-press menu

#### Scenario: Starring by touch

- **WHEN** the reader drags an entry row leftward past the commit threshold, or chooses the star toggle from the row's long-press menu
- **THEN** the entry's starred state toggles without opening the entry

#### Scenario: No tooltip-only labels

- **WHEN** a control's purpose is conveyed on desktop only by a tooltip
- **THEN** the same control conveys its purpose through visible text or an accessible name on a touch layout

#### Scenario: Hover styling does not stick

- **WHEN** a row is tapped on a touch device and the pointer moves away
- **THEN** the row does not retain the hover appearance

### Requirement: The reading pane is a separate screen on narrow viewports

On a narrow viewport, selecting an entry SHALL replace the entry list with the reading screen for that entry, and returning from the reading screen SHALL restore the entry list with the entry that is open shown as the selected row and brought into view if it is outside the visible rows. Marking an entry read on opening it SHALL behave as it does on desktop.

#### Scenario: Opening an entry

- **WHEN** the reader taps an entry row on a narrow viewport
- **THEN** the reading screen for that entry fills the viewport and the entry is marked read

#### Scenario: Returning to the list

- **WHEN** the reader leaves the reading screen
- **THEN** the entry list reappears with the entry that was open as its selected row, visible within the list

#### Scenario: Returning after reading several entries

- **WHEN** the reader opens an entry, swipes through several further entries, and then leaves the reading screen
- **THEN** the entry list is shown with the entry last read as its selected row and visible within the list

#### Scenario: The reading screen has its own actions

- **WHEN** the reading screen is shown
- **THEN** it offers a control to return to the list, the entry's read toggle, the entry's star toggle, and the link to the original article

### Requirement: Touch targets are large enough to hit

On a touch layout, every interactive control in the entry list, reading screen, sidebar, dialogs, and the row long-press menu SHALL present a touch target of at least 44 by 44 CSS pixels, and each entry row SHALL remain at least 44 CSS pixels tall, without shrinking the visible row content below its existing layout rules.

#### Scenario: Row controls meet the minimum

- **WHEN** a touch layout renders the entry list header's controls and the reading screen's controls
- **THEN** each control's tappable area is at least 44 by 44 CSS pixels

#### Scenario: The row's own targets meet the minimum

- **WHEN** a touch layout renders an entry row
- **THEN** the row is at least 44 CSS pixels tall and every item in its long-press menu is at least 44 by 44 CSS pixels

#### Scenario: Rows keep their proportions

- **WHEN** touch targets are enlarged on a touch layout
- **THEN** entry rows still show cover, title, excerpt, feed label, and time without the meta line overlapping adjacent rows
