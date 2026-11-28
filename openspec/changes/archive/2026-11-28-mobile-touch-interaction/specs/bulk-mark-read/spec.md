## MODIFIED Requirements

### Requirement: In-list batch actions mark only the entries the list has shown

The entry list SHALL offer two batch read actions: one in the list header that marks the entries rendered in the current list, and one per entry row that marks the rendered entries positioned above that row. Both SHALL affect only entries already rendered at the moment the action is activated, and MUST NOT mark entries that match the current selection but have not been loaded into the list. The row action SHALL exclude the row it was activated on. The row action SHALL be activated by its visible control where a pointer can hover, and by a rightward drag on the row or through the row's long-press menu on a touch layout.

#### Scenario: Header action on a partially loaded list

- **WHEN** 12,438 unread entries match the current selection, 50 of them are rendered, and the user activates the header batch action
- **THEN** those 50 entries become read and the remaining 12,388 stay unread

#### Scenario: Row action excludes its own row

- **WHEN** the user activates the row batch action on the 20th rendered entry
- **THEN** every rendered entry above it becomes read, and that entry together with everything below it keeps its read state

#### Scenario: Row action activated by a drag

- **WHEN** the user drags the 20th rendered entry rightward past the commit threshold
- **THEN** the row action is activated exactly as its visible control would activate it

#### Scenario: Row action absent when nothing is unread above

- **WHEN** no rendered entry above a row is unread
- **THEN** that row offers no batch action, and a rightward drag on it commits nothing

#### Scenario: Rendered entries that are already read

- **WHEN** the rendered entries above the row action's row include entries that are already read
- **THEN** only the unread ones among them are affected

#### Scenario: Batch read state persists

- **WHEN** the user marks entries read in a batch and restarts the application
- **THEN** those entries are still read

## REMOVED Requirements

### Requirement: Batch marking is immediate and one-way

**Reason**: The action is now also reachable by a horizontal drag on a touch layout, where a bulk, irreversible write sits one mis-gesture away from a scroll. Batch marking stays immediate but must be recoverable for a short window.

**Migration**: The batch actions keep their labels, their counts, and their position; activation still applies the change with no confirmation step. What changes is that an undo control is offered afterwards — see "Batch marking is immediate and reversible for a short window".

## ADDED Requirements

### Requirement: Batch marking is immediate and reversible for a short window

A batch action SHALL apply as soon as it is activated, without a confirmation step, and SHALL then offer an undo control for a short window. Choosing undo SHALL restore exactly the entries that batch marked to the read state they held before it, SHALL NOT touch entries the batch did not mark, and SHALL update the unread counts the sidebar shows. The undo control SHALL withdraw itself when its window passes, and the system SHALL still NOT provide a batch action that marks entries unread.

#### Scenario: No confirmation step

- **WHEN** the user activates any batch read action
- **THEN** the read state changes without a confirmation dialog

#### Scenario: Undo is offered after a batch mark

- **WHEN** a batch mark completes, whether it was activated from the header, a row, or a sidebar menu
- **THEN** an undo control is presented that states how many entries it would restore

#### Scenario: Undo restores the marked entries

- **WHEN** the user chooses undo within its window after a batch marked 12 entries read
- **THEN** exactly those 12 entries are unread again, their rows show as unread, and the sidebar's unread counts return to their previous values

#### Scenario: Undo leaves other entries alone

- **WHEN** the user chooses undo after a batch mark, and an entry that the batch did not mark was marked read or unread in the meantime
- **THEN** that entry's read state is unchanged by the undo

#### Scenario: Undo expires

- **WHEN** the undo window passes without the user choosing undo
- **THEN** the undo control is no longer offered and the batch mark stands

#### Scenario: Undo covers the gesture path

- **WHEN** the reader marks the entries above a row read by dragging that row rightward
- **THEN** the same undo control is offered and restores the same entries

#### Scenario: No batch unread action

- **WHEN** the user inspects every batch affordance in the application
- **THEN** none of them marks entries unread apart from undoing a batch that just ran
