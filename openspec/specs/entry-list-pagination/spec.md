# entry-list-pagination Specification

## Purpose

Governs how the entry list is loaded and scrolled: bounded batches appended through a stable cursor, a deterministic order, progress feedback, and on-demand retrieval of the selected entry's content.

## Requirements

### Requirement: The entry list loads in bounded batches

The system SHALL load at most 50 entries in the first batch and SHALL NOT load every matching entry up front. It SHALL request further batches as the reader reaches the end of the loaded rows, and SHALL stop once no matching entries remain.

#### Scenario: Many stored entries

- **WHEN** 12,438 entries match the current view and scope and the list is opened
- **THEN** exactly 50 entry rows are rendered, not all 12,438

#### Scenario: Reaching the end of the loaded rows

- **WHEN** the reader scrolls to the last loaded row and more entries match
- **THEN** another batch is requested and appended, and the rows already loaded stay in place

#### Scenario: Fewer entries than one batch

- **WHEN** fewer than 50 entries match the current view and scope
- **THEN** every matching entry is rendered and no further batch is requested

### Requirement: Batch loading is cursor-based and stable under refresh

The system SHALL continue each batch from the ordering position of the last loaded row, and MUST NOT skip or repeat an entry between batches. Reloading SHALL keep the loaded rows in the same relative order without duplication and SHALL NOT discard rows the reader has already scrolled through.

#### Scenario: Successive batches have no gaps

- **WHEN** the reader loads several batches in a row
- **THEN** the concatenation of those batches matches the list's ordering exactly, with no entry appearing twice and none missing in between

#### Scenario: A refresh does not disturb the loaded rows

- **WHEN** the reader has scrolled into older entries and a refresh stores new entries
- **THEN** the loaded rows remain in the same order with no duplicates, and the newly stored entries appear above them

### Requirement: Entries have a deterministic total order

The system SHALL order entries newest first using the publication time when one is known and the time the entry was stored otherwise, SHALL break ties by entry identity, and SHALL keep an entry's position stable for as long as it is stored. No entry may become unreachable or skipped while loading more.

#### Scenario: An entry without a publication date

- **WHEN** an entry has no publication date and the list is ordered
- **THEN** it is ordered by the time it was stored and remains reachable through further batch loading

#### Scenario: Entries sharing a publication date

- **WHEN** several entries carry the same publication time
- **THEN** their relative order is identical on every load, so no entry is shown twice or skipped between batches

### Requirement: List responses exclude entry content

The entry list response SHALL carry only the metadata needed to render a row and MUST NOT include stored entry HTML. The full content SHALL be requested separately for the entry being read.

#### Scenario: Loading a batch

- **WHEN** a batch of entries is loaded
- **THEN** the response contains titles, dates, and read/starred state but no entry bodies

#### Scenario: Reading an entry

- **WHEN** the user selects an entry from the list
- **THEN** that entry's content is fetched and displayed in the reading pane

### Requirement: The reading pane persists while the list loads

The system SHALL keep displaying the selected entry when further batches are loaded, and SHALL clear the reading pane only when the selected entry no longer exists.

#### Scenario: Loading more while reading

- **WHEN** the reader has an entry open and scrolls until another batch is loaded
- **THEN** the reading pane continues to display that entry

#### Scenario: The selected entry is deleted

- **WHEN** the feed containing the open entry is removed
- **THEN** the reading pane is cleared

### Requirement: Browsing progress is displayed

The system SHALL show, for the current view and scope, how many entries have been loaded so far and how many entries match in total. The loaded count SHALL reflect the rows on screen, and the total SHALL be the count for the current view and scope rather than a global count.

#### Scenario: Several batches loaded

- **WHEN** the reader has loaded three batches of 50 entries
- **THEN** the progress display reads "150 / 12,438 entries viewed" for a view that matches 12,438 entries

#### Scenario: Total follows the active filter

- **WHEN** the unread view is selected and 240 entries are unread
- **THEN** the total shown is 240, not the number of all stored entries

### Requirement: Changing view or scope starts over from the newest entries

The system SHALL clear the loaded rows and start from the newest batch whenever the selected feed, folder, global view, or all/unread tab changes, and SHALL keep the loaded rows when feeds are refreshed.

#### Scenario: Selecting a different feed after scrolling deep

- **WHEN** the reader has scrolled through several batches of "All entries" and selects a feed in the sidebar
- **THEN** that feed's newest entries are shown and no rows from the previous selection remain

#### Scenario: Refreshing keeps the reader's place

- **WHEN** the reader triggers a refresh after scrolling past the first batch
- **THEN** the rows they had loaded are still shown, with any newly stored entries above them
