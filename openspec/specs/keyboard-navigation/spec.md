# keyboard-navigation Specification

## Purpose

Makes the reader usable without a mouse: a single keyboard cursor over the entry list, Vim-style motions with counts, and single-key bindings for the actions the reader repeats hundreds of times per session, plus an in-app reference for the key map.

## Requirements

### Requirement: The entry list has a keyboard cursor

The application SHALL provide a keyboard cursor over the loaded entry rows. Moving the cursor SHALL open the row it lands on in the reading pane and SHALL mark that entry read, matching what selecting a row with the mouse already does. The cursor row SHALL be visually distinguishable from the other rows, and SHALL be the same row the reading pane is showing, so there is never a cursor row and an open row that differ.

#### Scenario: Moving down with no cursor

- **WHEN** the list holds entries and no entry has been opened yet, and the reader presses `j`
- **THEN** the first row becomes the cursor row, its content is displayed in the reading pane, and it is marked read

#### Scenario: Moving down with a cursor

- **WHEN** the cursor is on the third row and the reader presses `j`
- **THEN** the cursor moves to the fourth row, the reading pane shows that entry, and that entry is marked read

#### Scenario: Moving back up does not unmark

- **WHEN** the cursor is on a row that was marked read as the cursor passed it and the reader presses `k`
- **THEN** the cursor moves up one row and the entry it left is still read

#### Scenario: Moving past either end

- **WHEN** the cursor is on the first row and the reader presses `k`, or on the last loaded row while no further entries match and the reader presses `j`
- **THEN** the cursor stays where it is and no error is reported

#### Scenario: The list is empty

- **WHEN** no entries match the current view and scope
- **THEN** the navigation keys do nothing and no entry is opened

#### Scenario: Clicking a row places the cursor

- **WHEN** the reader clicks a row with the mouse and then presses `j`
- **THEN** the cursor moves from the clicked row, so switching between mouse and keyboard continues from where the reader is

### Requirement: Motions reach the ends of the list and accept a count

The application SHALL bind `gg` to the first loaded row and `G` to the last, and `Ctrl-d`/`Ctrl-u` to moving the cursor down or up by roughly half a screen. A motion SHALL accept a numeric prefix, so `5j` and `12G` are valid, and a count that would pass the last row SHALL stop at the last row rather than doing nothing.

#### Scenario: Jumping to the top of a long list

- **WHEN** the cursor is deep in the list and the reader presses `gg`
- **THEN** the cursor moves to the first loaded row and the list scrolls so that row is visible

#### Scenario: Jumping to the bottom of a long list

- **WHEN** the list has rows loaded and the reader presses `G`
- **THEN** the cursor moves to the last loaded row and the reading pane shows that entry

#### Scenario: Counting down

- **WHEN** the cursor is on the fifth row and the reader presses `5j`
- **THEN** the cursor moves to the tenth row

#### Scenario: A count larger than the remaining rows

- **WHEN** the cursor is on the fifth row of a ten-row list and the reader presses `20j`
- **THEN** the cursor ends on the tenth row

#### Scenario: Jumping to a numbered row

- **WHEN** the reader presses `12G`
- **THEN** the cursor moves to the twelfth row, or to the last row when fewer than twelve are loaded

#### Scenario: Half-page movement

- **WHEN** the reader presses `Ctrl-d`
- **THEN** the cursor moves down by about half the number of rows visible in the list, and the row it lands on is opened

### Requirement: The `g` prefix groups list-jumping keys

The application SHALL treat `g` as a prefix that starts a two-key sequence. `g g` SHALL reach the first row, `g a` SHALL select *All entries*, and `g s` SHALL select *Starred*. A sequence that never completes SHALL be discarded after a timeout, leaving the cursor and the selection unchanged, and SHALL NOT be interpreted as a single `g` command.

#### Scenario: Jumping to the starred view

- **WHEN** the reader presses `g` then `s`
- **THEN** the Starred view is selected and its list is loaded

#### Scenario: Jumping to all entries

- **WHEN** the reader presses `g` then `a` from any selection
- **THEN** the *All entries* view is selected and its list is loaded

#### Scenario: An incomplete sequence is dropped

- **WHEN** the reader presses `g` and then no key follows for the timeout
- **THEN** no navigation happens, the cursor stays on its row, and the pending prefix is forgotten

#### Scenario: An unknown suffix cancels the sequence

- **WHEN** the reader presses `g` and then a key that is not part of any sequence
- **THEN** that sequence is discarded and the key's other meaning, if it has none in this position, is not applied

#### Scenario: A count before the prefix

- **WHEN** the reader presses `3g` and then `g`
- **THEN** the count is not used to change the target of the jump, and the cursor still goes to the first row

### Requirement: The sidebar is reachable by two jumps only

The application SHALL bind `g a` and `g s` as the entire keyboard surface of the sidebar. It SHALL NOT provide a keyboard cursor over feeds or folders, nor keyboard bindings that select an individual feed, expand or collapse a folder, or run a feed's or folder's menu actions.

#### Scenario: No feed-level keyboard navigation

- **WHEN** the reader presses navigation keys while the list is empty
- **THEN** no feed becomes selected and the sidebar selection does not change

#### Scenario: The jumps ignore the current list

- **WHEN** the reader has a feed selected and presses `g s`
- **THEN** the Starred view replaces the feed's list

### Requirement: Existing actions are available as single keys

The application SHALL bind the row and list actions to single keys: `m` SHALL toggle the cursor row between read and unread, `s` SHALL toggle its starred state, `o` SHALL open the entry's original link in the browser, `v` SHALL switch the list between the All and Unread tabs where the filter is offered, `A` SHALL mark every unread row currently rendered read, and `r` SHALL refresh all feeds. Each binding SHALL have exactly the effect of the corresponding existing control, including its effect on unread counts and on the reading pane.

#### Scenario: Toggling read state from the keyboard

- **WHEN** the cursor is on an unread entry and the reader presses `m`
- **THEN** the entry becomes read, the sidebar's unread counts decrease, and pressing `m` again makes it unread again

#### Scenario: Starring from the keyboard

- **WHEN** the cursor is on an unstarred entry and the reader presses `s`
- **THEN** the entry becomes starred and the Starred view and its badge reflect that

#### Scenario: Opening the original article

- **WHEN** the cursor is on an entry that has a link and the reader presses `o`
- **THEN** the article is opened in the external browser and the application stays on that entry

#### Scenario: Switching the filter

- **WHEN** the reader presses `v` in a feed's list
- **THEN** the list switches to the other tab, the newly filtered list loads from its newest row, and the sidebar selection stays on that feed

#### Scenario: Marking the current page read

- **WHEN** the reader presses `A` on a triage list whose rendered rows include unread entries
- **THEN** exactly the rendered unread entries become read, as if the list header's batch control had been clicked

#### Scenario: Refreshing

- **WHEN** the reader presses `r`
- **THEN** every feed is refreshed and the loaded rows are kept, as if the refresh control had been clicked

#### Scenario: Actions that need a cursor row

- **WHEN** the reader presses `m`, `s`, or `o` while no entry is open
- **THEN** nothing happens and no request is made

### Requirement: The reading pane scrolls independently of the cursor

The application SHALL bind `J` and `K` to scrolling the reading pane down and up by one screen. Opening a different entry SHALL reset the reading pane's scroll position to the top. `J` and `K` SHALL NOT move the cursor or change which entry is open.

#### Scenario: Scrolling a long article

- **WHEN** the cursor is on an entry whose content is taller than the pane and the reader presses `J`
- **THEN** the pane scrolls down, the entry stays open, and the cursor does not move

#### Scenario: A newly opened entry starts at the top

- **WHEN** the reader has scrolled halfway through an article and then moves the cursor to another entry
- **THEN** the new entry's content is shown from its beginning

#### Scenario: Scrolling back up

- **WHEN** the reader presses `K` after scrolling into an article
- **THEN** the pane scrolls up by one screen

### Requirement: Keystrokes are suppressed while typing or while a dialog is open

The application SHALL ignore every navigation and action binding while a text input, text area, or editable element has focus, while the settings dialog or a feed edit dialog is open, and while a row menu is open. Keys pressed in those states SHALL reach the focused control with their normal meaning, and `Escape` SHALL keep closing the open dialog or menu.

#### Scenario: Typing a feed URL

- **WHEN** the feed dialog is open and the reader types `j` into the URL field
- **THEN** the character appears in the field and the cursor does not move and no entry is opened

#### Scenario: Letter keys do not leak through a dialog

- **WHEN** the settings dialog is open and the reader presses `r` or `A`
- **THEN** no refresh and no batch mark is triggered

#### Scenario: Escape still closes

- **WHEN** a dialog or a row menu is open and the reader presses `Escape`
- **THEN** it closes and previously typed text is unaffected

#### Scenario: Bindings are restored afterwards

- **WHEN** the reader closes a dialog and presses `j`
- **THEN** the cursor moves as usual

### Requirement: Modifier combinations are left alone

The application SHALL NOT consume a keystroke that carries the `Ctrl`, `Alt`, or `Meta` modifier except for `Ctrl-d` and `Ctrl-u`, so that shortcuts provided by the operating system, the window, or the webview keep working.

#### Scenario: An unrelated shortcut passes through

- **WHEN** the reader presses `Ctrl-c` or `Alt-j` in the entry list
- **THEN** the application performs no navigation or entry action for it

### Requirement: Burst input does not issue one request per repeat

Holding a navigation key SHALL move the cursor on every repeat without waiting for a request. Fetching the open entry's content and persisting read state SHALL be coalesced across a burst and applied once the burst settles, so that a burst of ten moves produces at most one content request for the row the cursor settled on, one read-state write covering every entry the cursor passed, and one refresh of the unread counts. Every entry the cursor passed SHALL end up read, not only the row it settled on.

#### Scenario: Holding down the down key

- **WHEN** the reader holds `j` until the cursor has moved ten rows and then releases it
- **THEN** the reading pane shows the tenth entry, all ten entries the cursor passed are read, and the unread counts reflect them

#### Scenario: The cursor never lags behind the key

- **WHEN** the reader presses `j` repeatedly
- **THEN** the cursor row and its read indicator update immediately, without waiting for a fetch or a write to complete

#### Scenario: Pending marks are not lost when the list is replaced

- **WHEN** the reader moves the cursor over several rows and immediately selects a different feed or switches the filter
- **THEN** the entries the cursor passed are still marked read

#### Scenario: Only the settled row's content is fetched

- **WHEN** the cursor passes over six rows quickly
- **THEN** the content of only the row the cursor is on is displayed, and the reading pane never shows an entry from a row the cursor has already left

### Requirement: Marking entries as the cursor passes does not reflow the list

Marking an entry read because the cursor moved onto it SHALL NOT remove the row, re-sort the list, or replace the loaded rows. The row SHALL stay in place and SHALL render as read, so that repeated movement never makes the list jump under the cursor. This applies only to cursor-driven marking; the batch actions keep replacing the marked rows in an unread list.

#### Scenario: Walking down an unread list

- **WHEN** the reader moves the cursor down ten rows of an unread feed
- **THEN** the ten rows stay in the list, rendered as read, and every other row keeps its position

#### Scenario: The batch action still refills

- **WHEN** the reader presses `A` on that same unread list
- **THEN** the marked rows are replaced by the next unread entries, as before

### Requirement: The cursor keeps its place across reloads

When the list is reloaded, refreshed, or extended with another batch, the cursor SHALL stay on the entry it was on when that entry is still in the list. When that entry is gone the cursor SHALL move to the row at the same position, or to the last row when the list has become shorter, with one exception: when the reader removes the subscription that fed the open entry, the cursor SHALL be cleared together with the reading pane, as it was before the keyboard existed. The cursor SHALL NOT jump back to the first row on a reload that keeps the reader's place.

#### Scenario: A refresh while reading

- **WHEN** the cursor is on the twentieth row and a refresh stores new entries above it
- **THEN** the cursor stays on the same entry, which has moved down the list

#### Scenario: An entry disappears from the list

- **WHEN** the cursor's entry is deleted elsewhere and a refresh reloads the list, which still holds rows
- **THEN** the cursor moves to the row that took its position and the reading pane shows that row

#### Scenario: The list becomes shorter than the cursor's position

- **WHEN** the cursor was on the tenth row and the reloaded list holds six
- **THEN** the cursor moves to the sixth row

#### Scenario: Removing the open entry's subscription

- **WHEN** the reader removes the feed the open entry belongs to, while the list still holds other feeds' rows
- **THEN** the cursor and the reading pane are cleared and the remaining rows keep their positions

#### Scenario: Changing the selection

- **WHEN** the reader selects a different feed
- **THEN** the cursor is cleared and the new list opens with no entry open

### Requirement: The cursor drives paging at the end of the loaded rows

When the cursor moves onto the last loaded row and further entries match the current view and scope, the application SHALL request the next batch. A move that reached the end SHALL be carried over to the newly appended rows rather than dropped, and when the next batch is empty the cursor SHALL stay on the last row. A jump to the end SHALL land on the last row that is already loaded and MUST NOT load every remaining batch to reach a row far below it; each further press at that row SHALL append one more batch and move the cursor to the new last row. One jump SHALL append at most one batch beyond the rows already loaded, however many times the list re-renders while that batch lands.

#### Scenario: Walking off the loaded rows

- **WHEN** the cursor is on the last loaded row while more entries match and the reader presses `j`
- **THEN** the next batch is loaded and the cursor continues onto the first row of that batch

#### Scenario: Jumping to the end of the loaded rows

- **WHEN** the list holds one loaded batch, 12,438 entries match, and the reader presses `G`
- **THEN** the cursor moves to the last loaded row, opens that entry, and at most one further batch is appended — the ordinary request made when the end of the loaded rows comes into view

#### Scenario: One jump, one batch

- **WHEN** a jump lands on the last loaded row and the batch it appends pushes the end of the list out of view
- **THEN** exactly one batch is appended, not one per render of the growing list

#### Scenario: Pressing G again at the end

- **WHEN** the cursor is on the last loaded row, more entries match, and the reader presses `G`
- **THEN** exactly one more batch is appended and the cursor moves to the last row of it

#### Scenario: Reaching the real end through repeated jumps

- **WHEN** the reader keeps pressing `G` until no further entries match
- **THEN** each press appends one batch, the final press finds nothing more, and the cursor stays on the last loaded row

#### Scenario: No further entries

- **WHEN** the cursor is on the last loaded row and no further entries match
- **THEN** the cursor stays there and no batch is requested

### Requirement: The key map is available in the application

The application SHALL show a help overlay listing every binding when the reader presses `?`, and SHALL close it again on `?` or `Escape`. While the overlay is open, no other binding SHALL act on the entry list.

#### Scenario: Opening the reference

- **WHEN** the reader presses `?`
- **THEN** an overlay appears listing the keyboard bindings

#### Scenario: Closing it

- **WHEN** the overlay is open and the reader presses either `?` or `Escape`
- **THEN** the overlay closes and the list is unchanged

#### Scenario: Keys do not act through the overlay

- **WHEN** the overlay is open and the reader presses `j` or `r`
- **THEN** neither the cursor nor a refresh is triggered

#### Scenario: The reference matches the bindings

- **WHEN** a binding described in the overlay is pressed
- **THEN** it performs the action the overlay describes
