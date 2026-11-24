## Purpose

Lets a reader acknowledge entries in batches — the ones currently on screen, or a whole subscription at once — instead of opening every entry to clear it from the unread list.

## ADDED Requirements

### Requirement: In-list batch actions mark only the entries the list has shown

The entry list SHALL offer two batch read actions: one in the list header that marks the entries rendered in the current list, and one per entry row that marks the rendered entries positioned above that row. Both SHALL affect only entries already rendered at the moment the action is activated, and MUST NOT mark entries that match the current selection but have not been loaded into the list. The row action SHALL exclude the row it was activated on.

#### Scenario: Header action on a partially loaded list

- **WHEN** 12,438 unread entries match the current selection, 50 of them are rendered, and the user activates the header batch action
- **THEN** those 50 entries become read and the remaining 12,388 stay unread

#### Scenario: Row action excludes its own row

- **WHEN** the user activates the row batch action on the 20th rendered entry
- **THEN** every rendered entry above it becomes read, and that entry together with everything below it keeps its read state

#### Scenario: Row action absent when nothing is unread above

- **WHEN** no rendered entry above a row is unread
- **THEN** that row offers no batch action

#### Scenario: Rendered entries that are already read

- **WHEN** the rendered entries above the row action's row include entries that are already read
- **THEN** only the unread ones among them are affected

#### Scenario: Batch read state persists

- **WHEN** the user marks entries read in a batch and restarts the application
- **THEN** those entries are still read

### Requirement: In-list batch actions are offered only on triage lists

The header and row batch actions SHALL be offered when the current list is a single feed's entries, a folder's entries, or the unread view of "All entries". They SHALL NOT be offered when "All entries" is showing its unfiltered view, nor in the Starred view.

#### Scenario: Offered inside a feed

- **WHEN** the user selects a feed in the sidebar
- **THEN** the list offers both the header batch action and the per-row batch action

#### Scenario: Offered in the global unread view

- **WHEN** the user selects "All entries" and then the Unread tab
- **THEN** the list offers both the header batch action and the per-row batch action

#### Scenario: Not offered in the unfiltered All entries view

- **WHEN** the user selects "All entries" and the All tab
- **THEN** neither batch read action is offered

#### Scenario: Not offered in the Starred view

- **WHEN** the user selects the Starred view
- **THEN** neither batch read action is offered

### Requirement: A subscription's whole backlog can be marked read from the sidebar

A feed's and a folder's sidebar row menu SHALL offer an action that marks every unread entry of that subscription or folder as read, including entries never loaded into the list. This action SHALL be available regardless of which view or selection the list currently shows, and SHALL be absent on the "All entries" and "Starred" rows. The action SHALL NOT be offered for a subscription or folder with no unread entries.

#### Scenario: Marking a whole feed read

- **WHEN** a feed holds 300 unread entries of which 50 are rendered and the user activates "mark all as read" from the feed's sidebar row menu
- **THEN** all 300 become read

#### Scenario: Marking a folder read from another view

- **WHEN** the list is showing the Starred view and the user activates "mark all as read" from a folder's sidebar row menu
- **THEN** every unread entry of that folder's feeds becomes read and the Starred list's starring is unaffected

#### Scenario: Global rows offer no whole-collection action

- **WHEN** the user inspects the "All entries" and "Starred" sidebar rows
- **THEN** neither offers a batch read action

#### Scenario: Nothing unread to mark

- **WHEN** a feed or folder has no unread entries
- **THEN** its row menu does not offer the whole-subscription batch action

### Requirement: Batch actions state how many entries they will affect

Each batch action SHALL display the number of entries it would mark read, derived from what that action can reach: the rendered unread entries for the header action, the rendered unread entries above the row for the row action, and the subscription's or folder's unread count for the sidebar action. The displayed number SHALL match the number of entries the action actually marks.

#### Scenario: Header action shows the rendered unread count

- **WHEN** the list renders 50 entries of which 12 are unread
- **THEN** the header batch action's label states 12

#### Scenario: Sidebar action shows the subscription's unread count

- **WHEN** a feed holds 300 unread entries
- **THEN** that feed's sidebar batch action states 300

#### Scenario: Counts agree with the effect

- **WHEN** a batch action stated 12 and is activated
- **THEN** exactly 12 entries become read

### Requirement: The list is restored after a batch mark

After a batch mark completes the list SHALL NOT be left with a gap. When the list is filtered to unread entries, the marked entries SHALL be replaced by the next unread entries, leaving a populated page that the reader does not have to scroll to reach. When the list also shows read entries, the marked entries SHALL keep their position and SHALL render as read, preserving the reader's scroll position.

#### Scenario: Unread list refills without scrolling

- **WHEN** the user marks the rendered page read while the list is filtered to unread entries and more unread entries match
- **THEN** the marked rows disappear, the next unread entries fill the page, and no scrolling is required

#### Scenario: Unread list when nothing remains

- **WHEN** the user marks the rendered page read while the list is filtered to unread entries and no unread entries remain
- **THEN** the list reports that no entries match

#### Scenario: Scroll position is preserved when read and unread entries share the list

- **WHEN** the user has scrolled deep into a feed's list and marks the rendered page read
- **THEN** the same entries stay in the list at the same positions, rendered as read

### Requirement: Batch marking leaves unread counts consistent

Every batch mark SHALL update the unread counts the sidebar shows for the affected feeds and folders, and SHALL keep an open entry's read toggle in step when that entry was marked.

#### Scenario: Feed and folder badges decrease

- **WHEN** a batch mark makes 12 entries of a feed read
- **THEN** that feed's unread badge decreases by 12 and its folder's badge decreases by the same amount

#### Scenario: Open entry follows the batch

- **WHEN** the entry open in the reading pane is included in a batch mark
- **THEN** the reading pane shows it as read and offers to mark it unread

### Requirement: Batch marking is immediate and one-way

A batch action SHALL apply as soon as it is activated, without a confirmation step, and SHALL NOT offer an undo affordance. The system SHALL NOT provide a batch action that marks entries unread.

#### Scenario: No confirmation step

- **WHEN** the user activates any batch read action
- **THEN** the read state changes without a confirmation dialog

#### Scenario: No undo offered

- **WHEN** a batch mark completes
- **THEN** no undo control is presented

#### Scenario: No batch unread action

- **WHEN** the user inspects every batch affordance in the application
- **THEN** none of them marks entries unread
