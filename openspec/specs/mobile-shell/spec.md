# mobile-shell Specification

## Purpose

Lets the reader use the same local library on an Android phone, where the screen is one pane wide and there is no mouse or keyboard: feeds live behind a drawer, the entry list and the reading screen take turns, and every action that desktop reaches through hover or a keystroke is reachable by touch.

## Requirements

### Requirement: The application runs as an Android app

The application SHALL be installable and launchable on Android devices and SHALL present the same reader frontend as the desktop build, storing its library in the device's application data directory. The library SHALL persist across application restarts and SHALL be independent of any other installation of the application.

#### Scenario: Launching the app on a device

- **WHEN** the installed application is started on an Android device
- **THEN** the reader frontend loads with the library stored on that device and no blank screen or resource-load error appears

#### Scenario: Data survives a restart

- **WHEN** the reader adds a feed, closes the application, and starts it again
- **THEN** the feed and its entries are still present

#### Scenario: A second installation has its own library

- **WHEN** the application is installed on a device while another installation already holds subscriptions
- **THEN** the new installation starts with its own empty library and the two do not share data

### Requirement: Wide viewports keep the three-column layout

When the viewport is wide enough to hold the sidebar, the entry list, and the reading pane side by side, the application SHALL render them as the three simultaneous columns it renders on desktop, with the same content and ordering. Narrow-viewport behavior SHALL NOT alter the wide layout, and the primary pointer's coarseness SHALL NOT change how many columns the wide layout shows.

#### Scenario: A wide window on desktop

- **WHEN** the application runs in a window wide enough for three columns
- **THEN** the sidebar, entry list, and reading pane are visible at the same time as before this change

#### Scenario: A wide viewport on a tablet

- **WHEN** the application runs on an Android device whose width holds three columns
- **THEN** the same three-column layout is presented

### Requirement: Narrow viewports show one pane at a time

When the viewport is too narrow for three columns, the application SHALL present exactly one pane at a time: the entry list by default, the reading pane once an entry is opened, and the sidebar only as an overlay. The entry list SHALL remain usable at phone widths, with each row still showing its cover, title, excerpt, feed label, and time according to the existing row rules.

#### Scenario: Opening the app on a phone

- **WHEN** the application is launched on a phone-sized viewport
- **THEN** the entry list for the current selection is shown, without the sidebar or reading pane beside it

#### Scenario: No horizontally clipped content

- **WHEN** a narrow viewport displays the entry list or the reading pane
- **THEN** the pane uses the full viewport width and no content requires horizontal scrolling

### Requirement: The sidebar is an overlay drawer on narrow viewports

On a narrow viewport the sidebar SHALL be reachable from a control that is always visible in the entry list header, SHALL open over the list with a dimmed backdrop, and SHALL close on backdrop tap, on selection of a feed, folder, or global view, or on the existing sidebar toggle. Choosing a selection SHALL leave the reader on the entry list for that selection.

#### Scenario: Opening the drawer

- **WHEN** the reader activates the navigation control on a narrow viewport
- **THEN** the sidebar appears over the entry list with a backdrop behind it

#### Scenario: Selecting a feed closes the drawer

- **WHEN** the drawer is open and the reader activates a folder or feed row
- **THEN** the drawer closes and the entry list shows that selection's entries

#### Scenario: Dismissing the drawer without selecting

- **WHEN** the drawer is open and the reader taps the backdrop
- **THEN** the drawer closes and the current selection is unchanged

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

### Requirement: Back navigation unwinds the narrow layout

On a narrow viewport, the system back action SHALL close the reading screen when it is open, otherwise close the drawer when it is open, and otherwise leave the application. The same unwinding SHALL apply to an open dialog or overlay, which SHALL close first.

#### Scenario: Back from the reading screen

- **WHEN** the reading screen is open and the reader performs the system back action
- **THEN** the reading screen closes and the entry list is shown

#### Scenario: Back from the drawer

- **WHEN** the entry list is shown with the drawer open and the reader performs the system back action
- **THEN** the drawer closes and the entry list remains

#### Scenario: Back from an open dialog

- **WHEN** a dialog is open on a narrow viewport and the reader performs the system back action
- **THEN** the dialog closes and the underlying view is unchanged

#### Scenario: Back at the top level

- **WHEN** the entry list is shown with no drawer, screen, or dialog open and the reader performs the system back action
- **THEN** the application leaves, as the platform expects

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

### Requirement: Dialogs fit and dismiss on narrow viewports

On a narrow viewport, the settings, feed-edit, folder, and confirmation dialogs SHALL fit within the viewport, SHALL scroll internally when their content exceeds the available height, and SHALL be dismissible through their close control and through the system back action. Confirmation requests that use prompts SHALL be usable without a physical keyboard.

#### Scenario: Opening settings on a phone

- **WHEN** the reader opens the settings dialog on a phone-sized viewport
- **THEN** the dialog fits within the viewport, its actions are reachable, and no control is clipped

#### Scenario: A tall dialog scrolls

- **WHEN** a dialog's content is taller than the viewport
- **THEN** the dialog's content scrolls inside the dialog and the underlying view does not scroll

#### Scenario: Confirming from touch

- **WHEN** the reader removes a feed and a confirmation is required
- **THEN** the confirmation offers tap-able accept and cancel choices and respects the system back action

### Requirement: Content respects system insets

On Android, the narrow layout SHALL keep its topmost and bottommost content clear of the status bar, the navigation gesture area, and the on-screen keyboard, so that headers, list footers, and focused inputs are not obscured.

#### Scenario: Header below the status bar

- **WHEN** the application is shown on a device with a status bar
- **THEN** the entry list header and the reading screen header are fully visible below it

#### Scenario: Inputs stay visible while typing

- **WHEN** the reader focuses the add-feed field and the on-screen keyboard appears
- **THEN** the field and the action that submits it remain visible above the keyboard

### Requirement: Refreshing is available on resume

On a narrow viewport the application SHALL keep the existing manual refresh actions reachable, and SHALL refresh the feeds of the current selection when the application returns to the foreground after being backgrounded, without blocking interaction while the refresh runs and without refreshing when the application has only been interrupted by an overlay or dialog.

#### Scenario: Returning to the app refreshes

- **WHEN** the application is brought back to the foreground after being in the background
- **THEN** the current selection's feeds are refreshed and new entries appear when the refresh completes

#### Scenario: A failed refresh is reported without blocking

- **WHEN** a resume refresh fails for one or more feeds
- **THEN** the failure is reported as the existing refresh reporting does and the list remains usable

#### Scenario: No background work while stopped

- **WHEN** the application is not in the foreground
- **THEN** it schedules no fetch of its own and performs no reading-loop work

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
