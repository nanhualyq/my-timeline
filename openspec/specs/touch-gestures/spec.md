# touch-gestures Specification

## Purpose

Defines the horizontal drag vocabulary of the touch layout — what a leftward or rightward drag does on an entry row and in the reading pane — together with the mechanics that keep those drags from colliding with vertical scrolling, tapping, the operating system's edge gestures, and assistive technology.

## Requirements

### Requirement: A horizontal drag on an entry row acts on that row

On a touch layout, dragging an entry row horizontally SHALL claim the drag, and the action it commits SHALL follow the direction of the drag: a leftward drag SHALL toggle that row's star, and a rightward drag SHALL mark the rendered entries above that row as read, excluding that row. Releasing before the drag passes the commit threshold SHALL commit nothing, and the row SHALL return to its resting position. The same drag beginning where a pointer can hover SHALL NOT act on the row.

#### Scenario: Starring by dragging left

- **WHEN** the reader drags an unstarred row to the left past the commit threshold and releases
- **THEN** the entry becomes starred, the row shows itself as starred, and the entry does not open

#### Scenario: Unstarring by dragging left again

- **WHEN** the reader drags an already-starred row to the left past the commit threshold and releases
- **THEN** the entry is no longer starred

#### Scenario: Marking the entries above by dragging right

- **WHEN** the reader drags a row to the right past the commit threshold and releases, with unread entries above it
- **THEN** every rendered unread entry above that row becomes read and that row keeps its own read state

#### Scenario: A short drag commits nothing

- **WHEN** the reader drags a row horizontally without passing the commit threshold and releases
- **THEN** no action is taken and the row returns to its resting position

#### Scenario: A pointer layout keeps its controls

- **WHEN** a row is displayed where a pointer can hover
- **THEN** neither horizontal drag acts on the row, and the row's actions remain available through its existing controls

### Requirement: A drag in progress shows the action before it commits

While a horizontal drag on a row is in progress, the row SHALL follow the drag and SHALL reveal the action the drag would commit, including the number of entries a rightward drag would mark read. The revealed action SHALL update when the drag changes direction, and SHALL disappear if the drag is cancelled.

#### Scenario: The drag reveals its action

- **WHEN** the reader drags a row halfway to the commit threshold
- **THEN** the action that releasing would commit is visible before the release

#### Scenario: The revealed count matches the effect

- **WHEN** a rightward drag reveals that it would mark 3 entries read and the reader releases past the threshold
- **THEN** exactly 3 entries become read

#### Scenario: Changing direction changes the revealed action

- **WHEN** the reader drags left past the threshold and then continues the same touch rightward
- **THEN** the revealed action becomes the one the rightward drag commits, and the leftward action is not committed

#### Scenario: Reversing the drag does not commit the abandoned action

- **WHEN** the reader drags left past the threshold into the rightward direction and then releases
- **THEN** the entry's star state is unchanged by the abandoned leftward drag

### Requirement: A gesture claims the drag only after locking its direction

A horizontal gesture on a row SHALL take over the drag only once the movement has locked its direction as horizontal; until then the movement SHALL remain available to the list for vertical scrolling. A touch that moves less than the gesture's movement slop SHALL be treated as a tap and SHALL open the entry as a tap does.

#### Scenario: Scrolling a row still scrolls

- **WHEN** the reader drags a row vertically
- **THEN** the list scrolls and no row action is committed

#### Scenario: A slightly diagonal downward drag scrolls

- **WHEN** the reader drags a row downwards with a small horizontal component
- **THEN** the list scrolls and the row action is not committed

#### Scenario: A tap still opens the entry

- **WHEN** the reader touches a row and releases it without moving beyond the movement slop
- **THEN** the entry opens exactly as a tap opens it

### Requirement: Row gestures leave the screen edges to the operating system

A horizontal row gesture SHALL NOT be claimed when the touch begins within the gutter that the operating system reserves for its own edge gestures, so that the system back action continues to work from any row.

#### Scenario: A drag from the left edge is not a batch mark

- **WHEN** the reader begins a rightward drag inside the reserved edge gutter
- **THEN** the row does not mark the entries above it read

### Requirement: A long press on an entry row offers the same row actions

On a touch layout, a long press on an entry row SHALL present a menu offering the row's star toggle and the action that marks the rendered entries above the row read, so that both actions remain reachable without a horizontal drag. Choosing an item SHALL commit it and close the menu; dismissing the menu SHALL change nothing.

#### Scenario: The menu offers both row actions

- **WHEN** the reader long-presses an entry row that has unread entries above it
- **THEN** the menu offers the star toggle and the action that marks those entries read

#### Scenario: Choosing from the menu commits the action

- **WHEN** the reader long-presses a row and chooses the star toggle
- **THEN** the entry's star state changes and the entry does not open

#### Scenario: Dismissing the menu changes nothing

- **WHEN** the menu is open and the reader dismisses it without choosing
- **THEN** the row's star and read state are unchanged and the entry list remains

#### Scenario: The menu is reachable where a drag is not

- **WHEN** an assistive technology consumes horizontal drags on the entry list
- **THEN** the row's star toggle and batch mark remain reachable through the long-press menu

### Requirement: Horizontal swipes in the reading pane switch entries

On a touch layout, the reading pane SHALL switch to the next entry on a leftward swipe and to the previous entry on a rightward swipe, using the same movement the pointer layout's next and previous commands use, including marking the entry it opens as read and continuing into the next batch of entries when the swipe walks past the loaded rows. A swipe at either end of the available entries SHALL leave the open entry unchanged.

#### Scenario: Swiping to the next entry

- **WHEN** the reader swipes left in the reading pane
- **THEN** the next entry's article is shown and that entry is marked read

#### Scenario: Swiping to the previous entry

- **WHEN** the reader swipes right in the reading pane
- **THEN** the previous entry's article is shown and that entry is marked read

#### Scenario: Swiping past the loaded rows continues the list

- **WHEN** the reader swipes to the next entry at the last loaded row while more entries match the current list
- **THEN** the next batch is loaded and the following entry is shown

#### Scenario: Swiping at the end keeps the open entry

- **WHEN** the reader swipes to the next entry while the open entry is the last one available
- **THEN** the open entry stays open and no error is reported

### Requirement: A swipe in the reading pane leaves article content usable

The reading pane's entry swipe SHALL NOT claim vertical movement, so the article still scrolls, and SHALL preserve the ability to select text in the article.

#### Scenario: Vertical drag scrolls the article

- **WHEN** the reader drags vertically inside an article
- **THEN** the article scrolls and the open entry does not change

#### Scenario: Text stays selectable

- **WHEN** the reader long-presses inside an article to select text
- **THEN** text selection starts and the open entry does not change

### Requirement: The swipe vocabulary is taught where it cannot be discovered

The application SHALL make the row gestures discoverable on a touch layout through the long-press menu, and the reading pane's entry swipe SHALL be discoverable without requiring a drag first.

#### Scenario: The row's actions are discoverable without gesturing

- **WHEN** a reader who never drags a row long-presses it
- **THEN** the row's actions are offered

#### Scenario: The reading screen indicates it can be swiped

- **WHEN** the reading screen is shown on a touch layout
- **THEN** its neighbouring entries are indicated in the screen's own controls, so the swipe is not the only way to learn that switching exists
