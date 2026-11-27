## MODIFIED Requirements

### Requirement: User can import an OPML file

The system SHALL let the user choose an `.opml` file and import it: top-level and nested outlines that carry a feed URL SHALL become subscriptions, and outlines without a feed URL that contain feed outlines SHALL become folders; since folders are flat, feeds from nested folder outlines SHALL be assigned to their top-level ancestor folder. The import SHALL report how many feeds were added and how many were skipped. The file SHALL be chosen through the platform's file picker, and the system SHALL accept whatever reference the picker returns — a filesystem path or, where the platform provides one, an opaque document reference — without requiring the reader to copy or move the file first.

#### Scenario: Importing a fresh OPML file

- **WHEN** the user imports an OPML file containing a folder outline "Tech" with two feed outlines inside it, and neither URL is subscribed
- **THEN** a folder "Tech" is created containing both feeds, and the import reports two feeds added

#### Scenario: Importing an OPML file with nested folders

- **WHEN** the user imports an OPML file where a folder outline contains another folder outline containing feeds
- **THEN** the feeds are assigned to the top-level ancestor folder, since folders are flat

#### Scenario: Importing an invalid file

- **WHEN** the user chooses a file that is not well-formed OPML
- **THEN** no folders or feeds are created and the user sees an error message

#### Scenario: Importing through a platform document picker

- **WHEN** the user chooses an OPML file on a platform whose picker returns an opaque document reference rather than a filesystem path
- **THEN** the file is read through that reference, its feeds and folders are created as for a path, and the reported outcome is the same

#### Scenario: A file that cannot be read

- **WHEN** the reference the picker returns cannot be opened or read
- **THEN** no folders or feeds are created and the user sees an error message naming the file

### Requirement: User can export an OPML file

The system SHALL let the user export all subscriptions as an `.opml` file, with each feed placed inside an outline for its folder; uncategorized feeds SHALL be exported as top-level feed outlines. The exported file SHALL be valid OPML that can be re-imported without loss of folder grouping. The destination SHALL be chosen through the platform's file picker, and the system SHALL write through the reference the picker returns, including opaque document references on platforms that provide them, and SHALL report an error naming the destination when the write fails.

#### Scenario: Exporting subscriptions with folders

- **WHEN** the user exports a subscription set where feed X is in folder "Tech" and feed Y has no folder
- **THEN** the file contains a "Tech" outline wrapping feed X and feed Y as a top-level feed outline

#### Scenario: Round-trip export then import

- **WHEN** the user exports their subscriptions and re-imports the file into an empty application state
- **THEN** the same folders, folder grouping, and feed URLs are present afterwards

#### Scenario: Exporting through a platform document picker

- **WHEN** the user exports subscriptions on a platform whose picker returns an opaque document reference
- **THEN** the OPML content is written through that reference and the export reports the number of feeds exported

#### Scenario: A destination that cannot be written

- **WHEN** the destination the picker returns cannot be written
- **THEN** the user sees an error message naming the destination and the subscriptions are unchanged
