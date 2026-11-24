# entry-row-presentation Specification

## Purpose

Defines what a single entry row in the entry list shows — cover thumbnail, title, excerpt, feed label, and relative time — and how each detail behaves when the underlying data is missing.

## Requirements

### Requirement: An entry row shows cover, title, excerpt, feed label, and time

Each entry row SHALL display, when available, a cover thumbnail, the entry title, a plain-text excerpt, the display name of the feed that provided the entry, and the entry's time. A row SHALL additionally keep its existing read indicator and star control. The feed display name SHALL be the same name the sidebar shows for that feed, including any custom title the reader has set.

#### Scenario: An entry with every detail available

- **WHEN** an entry with a cover image, an excerpt, and a resolvable feed is listed
- **THEN** its row shows the cover thumbnail, the title, the excerpt, the feed's display name, the relative time, and the star control

#### Scenario: A feed the reader has renamed

- **WHEN** an entry belongs to a feed for which the reader has set a custom title
- **THEN** the row's feed label shows that custom title, not the title supplied by the feed

### Requirement: A row's height does not depend on which details it has

Rows SHALL keep a uniform height whether or not an entry has a cover image or an excerpt. A row without a cover SHALL NOT reserve an empty image slot, and a row without an excerpt SHALL NOT render an empty placeholder line. Loading a row's cover image SHALL NOT change the height of any row or shift the position of rows already displayed.

#### Scenario: Mixed entries in one list

- **WHEN** the list contains entries with covers and entries without
- **THEN** every row has the same height and no row shows an empty image frame

#### Scenario: An entry without an excerpt

- **WHEN** an entry has no derivable excerpt text
- **THEN** its row shows the title and feed label with no empty excerpt line

#### Scenario: Covers arriving late

- **WHEN** cover images finish loading after the rows are already displayed
- **THEN** no row changes height and the rows already on screen do not move

### Requirement: Long titles and excerpts are truncated, not wrapped indefinitely

The title and the excerpt SHALL each be limited to at most two lines of text and SHALL be truncated with an ellipsis when longer. The full title SHALL remain readable through the reading pane after the entry is selected.

#### Scenario: A very long title

- **WHEN** an entry's title is longer than two lines
- **THEN** the row shows the first two lines ending in an ellipsis and the row height stays uniform

### Requirement: A cover image that cannot be displayed is hidden

When a row's cover image fails to load, the row SHALL hide the broken image and display no placeholder, keeping its height and the rest of its content unchanged.

#### Scenario: A cover URL that no longer resolves

- **WHEN** a listed entry's cover image returns an error
- **THEN** the row renders without an image, with the same height, and shows no broken-image indicator

### Requirement: Time is shown as a short relative interval

The row SHALL show the entry's time as a short relative interval rather than an absolute date: a "just now" label for less than 45 seconds, a minute count below one hour, an hour count below one day, a day count below seven days, and a short absolute date beyond that. A time in the future SHALL be shown as "just now". The relative label SHALL be updated while the list is displayed, so it does not become stale. The full timestamp SHALL be available from the row without opening the entry.

#### Scenario: Recent entry

- **WHEN** an entry was published 20 minutes ago
- **THEN** its row shows a "20m"-style label

#### Scenario: Entry older than a week

- **WHEN** an entry was published a month ago
- **THEN** its row shows a short absolute date instead of a day count

#### Scenario: Full timestamp on hover

- **WHEN** the reader hovers the relative time of a row
- **THEN** the entry's full publication timestamp is revealed

#### Scenario: Entry with no publication time

- **WHEN** an entry carries no publication time
- **THEN** its row's relative time is based on the time the entry was stored

#### Scenario: Label ages while the list stays open

- **WHEN** the list remains open as time passes
- **THEN** the relative labels grow without the reader reloading or scrolling

### Requirement: The feed label is omitted where it repeats the current selection

The feed label SHALL be shown in the global views (all entries and starred) and in a folder's view. It SHALL be omitted when the list is scoped to a single feed, because the sidebar already identifies that feed.

#### Scenario: All-entries view

- **WHEN** the reader is browsing all entries
- **THEN** each row shows the feed label

#### Scenario: Single-feed view

- **WHEN** the reader selects one feed in the sidebar
- **THEN** the rows omit the feed label and no space is left for it
