## ADDED Requirements

### Requirement: Stored entries carry a plain-text excerpt and a cover image URL

For each entry it stores, the system SHALL persist a short plain-text excerpt and, when one can be derived, a cover image URL. The excerpt SHALL be derived from the feed's own summary when the feed provides one, and otherwise from the entry's stored content; excerpts MUST be plain text with markup removed and consecutive whitespace collapsed, and SHALL be truncated to a bounded length at a word boundary. A cover image URL SHALL be derived from the entry's media or image attachments, or from the first image in its content, and SHALL be an absolute URL resolved against the entry's link or the feed's site address. The system SHALL NOT download or store image bytes.

#### Scenario: Feed supplies a summary

- **WHEN** a fetched entry carries a summary and full content
- **THEN** its stored excerpt is derived from the summary, not from the content

#### Scenario: Feed supplies only content

- **WHEN** a fetched entry carries content but no summary
- **THEN** its stored excerpt is derived from the beginning of that content with markup removed

#### Scenario: Cover from a media attachment

- **WHEN** a fetched entry carries a media thumbnail or an image enclosure
- **THEN** its stored cover image URL is that image's URL

#### Scenario: Cover from content markup

- **WHEN** a fetched entry carries no media attachment but its content contains an image
- **THEN** its stored cover image URL is the first such image, resolved to an absolute URL

#### Scenario: Relative image reference

- **WHEN** a cover image reference is relative rather than absolute
- **THEN** it is stored as an absolute URL resolved against the entry's link or the feed's site address

#### Scenario: No derivable cover

- **WHEN** a fetched entry carries no image material at all
- **THEN** the entry is stored successfully with no cover image URL

### Requirement: Entries stored before row details existed are backfilled

The system SHALL, once, derive an excerpt and a cover image URL for entries stored before this data was captured, using their already-stored content, so the list gains row details without re-fetching every feed. The backfill SHALL NOT alter entry identity, read state, starred state, stored content, or publication times, and SHALL NOT need to run again once it has completed for an entry.

#### Scenario: Upgrading an existing database

- **WHEN** the application starts against a database whose entries predate cover and excerpt storage
- **THEN** every such entry receives an excerpt, and a cover URL when its stored content contains an image

#### Scenario: Re-running does not duplicate work

- **WHEN** the application starts again after a completed backfill
- **THEN** a backfilled entry is not processed a second time

## MODIFIED Requirements

### Requirement: Entries are deduplicated per feed

The system SHALL store each entry at most once per feed, using the entry's GUID as the identity and falling back to its link when no GUID is present. Re-fetching a feed SHALL NOT create duplicate entries; updated entry content SHALL update the existing record, together with the excerpt and cover image URL derived from that content.

#### Scenario: Re-fetching an unchanged feed

- **WHEN** a feed is refreshed and contains entries already stored
- **THEN** no duplicate entries are created

#### Scenario: Entries without GUIDs are identified by link

- **WHEN** a feed entry has no GUID but has a link that was already stored for that feed
- **THEN** the entry is not stored again

#### Scenario: An entry gains a cover on refresh

- **WHEN** a feed is refreshed and an already-stored entry now carries an image where it previously had none
- **THEN** that entry's stored cover image URL and excerpt are updated in place without creating a new entry
