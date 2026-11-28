# sync-document Specification

## Purpose

Defines the portable document that carries a reader's shared library between installations — subscriptions, folder organization, stars, read marks, and removals — together with the rules that make two independently edited copies merge back into one without losing either side's intent.

## Requirements

### Requirement: The sync document carries shared library state by stable key

The system SHALL define a versioned sync document that describes the parts of a library more than one installation shares: folders, feeds with their folder assignment and custom title, per-entry stars and read marks, and the removal of feeds and folders. Every record in the document SHALL be identified by a key that survives copying to another installation, never by a local integer id.

The document SHALL NOT contain entry bodies, entry headlines, entry excerpts, cover images, fetch timestamps, or fetch errors. Those are re-acquired from each feed by every installation.

#### Scenario: Exported document holds shared state only

- **WHEN** the reader exports a sync document from a library with feeds, folders, stars, and read entries
- **THEN** the document identifies each feed by its feed URL, each folder by its own generated identifier, and each starred or read entry by the pairing of its feed URL and its entry identifier
- **AND** the document contains no entry headlines, entry bodies, or fetch timestamps

#### Scenario: Local numeric ids do not appear in the document

- **WHEN** the reader exports a document and then imports it into an installation whose local rows have different numeric ids
- **THEN** every folder, feed, star, and read mark still applies to the intended item

#### Scenario: Document declares its own format version

- **WHEN** the reader exports a document
- **THEN** the document states the format version it was written with

### Requirement: Two documents merge into a single document

The system SHALL merge two sync documents into one document that represents the union of both. Merging SHALL be order-independent and idempotent: merging A with B in either order yields the same result, and merging a document with itself changes nothing.

Subscriptions SHALL union by feed URL. A folder present in either document SHALL be present in the result. For each independently settable value — a feed's custom title, a feed's folder assignment, and an entry's star and read state — the merge SHALL resolve to a single value.

#### Scenario: Subscriptions from both sides survive

- **WHEN** one document lists feeds A and B and another lists feeds B and C
- **THEN** the merged document lists feeds A, B, and C, with B appearing once

#### Scenario: Merging is symmetric

- **WHEN** the same two documents are merged in the opposite order
- **THEN** the result is identical
- **AND** merging the result with either input again leaves it unchanged

#### Scenario: A star set on either side is kept

- **WHEN** one document marks an entry starred and the other does not mention that entry
- **THEN** the merged document marks it starred

### Requirement: Conflicting values resolve by recorded order, not by clock time

Each independently settable value SHALL carry a monotonically increasing ordering value, and a conflict between two different values for the same field SHALL resolve to the one with the higher ordering value. The system SHALL NOT compare wall-clock timestamps to decide which of two values is newer.

The ordering value of an installation's own edits SHALL be strictly greater than any ordering value it has observed from a merged document, so that an edit made after seeing a remote value always wins over that value.

An absent value and a value that has never been set SHALL be treated as different from an explicit "false", so that clearing a star or marking an entry unread can be propagated instead of being silently reinstated by the other side.

#### Scenario: Later edit wins over an observed remote value

- **WHEN** an installation imports a document in which a feed's custom title is "Old name" and then the reader renames that feed to "New name"
- **THEN** exporting and merging that document elsewhere yields "New name"

#### Scenario: Equal values do not disturb the ordering value

- **WHEN** a merge finds the same value on both sides for a field
- **THEN** the resulting document keeps that value unchanged

#### Scenario: Clearing a star propagates

- **WHEN** one document clears a star that the other document still contains as set
- **THEN** the merged result reflects the clearing when the clearing carries the higher ordering value

#### Scenario: Skewed clocks do not change the outcome

- **WHEN** two installations with system clocks set far apart edit the same field
- **THEN** the merged result is decided by the recorded ordering values, not by the clock readings

### Requirement: Removal propagates and is not silently undone

Removing a feed or folder SHALL record a tombstone for it in the document. A tombstone SHALL outrank a feed or folder that another installation still lists, so that a removal made on one installation is not reverted by importing a document from an installation that has not yet seen it.

A tombstone SHALL NOT outrank the reader's own local action on the installation performing it: adding a feed or a folder again SHALL succeed, and the new addition SHALL outrank the older tombstone.

#### Scenario: Removal survives a merge with a stale copy

- **WHEN** an installation removes a feed and later merges with a document exported before the removal
- **THEN** the feed does not reappear

#### Scenario: Adding a feed again after removal

- **WHEN** the reader adds a feed that has a tombstone from an earlier removal, on the same installation
- **THEN** the feed is added and is present in the exported document

#### Scenario: Import does not remove local data

- **WHEN** the reader imports a document that does not mention feeds the local library already has
- **THEN** the local feeds and folders remain, and no local data is removed by the import

### Requirement: Shared read marks are bounded, stars are not

Read marks SHALL be shared only for entries whose ordering position falls inside a bounded recent window; stars SHALL be shared for entries regardless of age. A read mark for an entry older than the window SHALL NOT be recorded in the document, and a document SHALL drop read marks that have fallen out of the window.

The window SHALL be expressed as a calendar day recorded in the document, not as a distance from the current time, so that every installation derives the same window from the same document.

Tombstones SHALL be retained for longer than read marks, so that an installation that has been out of use for a while still learns about removals.

#### Scenario: Old read marks are not shared

- **WHEN** the reader marks an entry older than the window as read
- **THEN** the exported document does not contain a read mark for that entry
- **AND** the local library still records it as read

#### Scenario: Stars survive the window

- **WHEN** the reader stars an entry older than the window
- **THEN** the exported document contains the star

#### Scenario: Two installations agree on the window

- **WHEN** two installations with different system clocks import the same document
- **THEN** both compute the same window from it

#### Scenario: A long-absent installation learns about removals

- **WHEN** an installation imports a document after a period longer than the read window but shorter than the tombstone retention
- **THEN** the removals recorded in that document still take effect

### Requirement: A mark can be recorded for an entry that has not arrived yet

The system SHALL record a star or read mark independently of whether the entry it refers to exists in the local library, and SHALL apply a recorded mark when the entry arrives in a later fetch.

#### Scenario: Star imported before its feed is fetched

- **WHEN** an imported document marks an entry starred, and the local library has that feed but has not yet fetched the entry
- **THEN** the entry is not lost from the import
- **AND** after the feed is fetched the entry is listed as starred

#### Scenario: Mark recorded locally before the entry arrives

- **WHEN** a mark is recorded for an entry that is not present locally, and that entry is fetched later
- **THEN** the entry appears with the recorded mark applied

### Requirement: Reader exports and imports a sync document from the Settings dialog

The system SHALL provide an export action that writes the current shared state as a sync document to a location the reader picks, and an import action that reads a document from a location the reader picks and merges it into the local library.

The import SHALL report a summary of what it applied: how many folders, subscriptions, stars, and read marks were added or changed, and how many entries were recorded for later arrival. A document that cannot be read, or whose format version the system does not support, SHALL be rejected with a message naming the problem, and SHALL leave the local library unchanged.

Importing SHALL NOT fetch any feed and SHALL NOT remove or overwrite settings the reader has already made locally.

#### Scenario: Export writes to the picked location

- **WHEN** the reader chooses Export Sync Document and picks a file location
- **THEN** a sync document is written there and the dialog reports that the export succeeded

#### Scenario: Import reports what it applied

- **WHEN** the reader imports a document containing two new feeds, one folder, and three stars
- **THEN** the dialog reports those counts
- **AND** the sidebar shows the two feeds and the folder

#### Scenario: Unsupported version is rejected

- **WHEN** the reader imports a document whose format version this build does not support
- **THEN** a message names the version problem
- **AND** the local library is unchanged

#### Scenario: Cancelling the picker changes nothing

- **WHEN** the reader dismisses the file picker without choosing a file
- **THEN** no export or import happens and no message is reported as an error

#### Scenario: Import does not fetch feeds

- **WHEN** the reader imports a document containing a feed not present locally
- **THEN** the feed is added to the subscription list
- **AND** no network request is made for it by the import itself

#### Scenario: Importing the same document twice is harmless

- **WHEN** the reader imports the same document twice
- **THEN** the second import reports no further changes to the library

### Requirement: Rendering the same shared state produces the same text

The system SHALL render a shared library state as document text deterministically: records SHALL be emitted in a stable order, and re-rendering an unchanged state SHALL produce byte-identical text.

This SHALL hold for text produced by exporting a library and for text produced by merging documents, so that holding two renderings side by side and comparing their text is a reliable answer to "is anything different".

#### Scenario: Exporting an unchanged library twice produces identical text

- **WHEN** the reader exports a document, changes nothing, and exports again
- **THEN** the two pieces of text are byte-identical

#### Scenario: Export order does not depend on the order things were created

- **WHEN** two libraries hold the same subscriptions, folders, stars, and read marks, created in a different order
- **THEN** their exported documents are byte-identical

#### Scenario: A merged document and an exported document agree

- **WHEN** a document is merged from two copies and re-rendered from the merged library
- **THEN** both renderings are byte-identical
