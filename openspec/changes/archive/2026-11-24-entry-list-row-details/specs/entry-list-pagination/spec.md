## MODIFIED Requirements

### Requirement: List responses exclude entry content

The entry list response SHALL carry only the metadata needed to render a row: titles, times, feed identity, read and starred state, a short derived plain-text excerpt, and a cover image URL. It MUST NOT include the stored entry HTML or any other entry body. The full content SHALL be requested separately for the entry being read.

#### Scenario: Loading a batch

- **WHEN** a batch of entries is loaded
- **THEN** the response contains titles, times, feed identity, read/starred state, excerpts, and cover image URLs, but no entry bodies

#### Scenario: Reading an entry

- **WHEN** the user selects an entry from the list
- **THEN** that entry's content is fetched and displayed in the reading pane

#### Scenario: Excerpt is bounded

- **WHEN** an entry's stored content is a long article
- **THEN** the excerpt in the list response is a short plain-text fragment rather than the full article text

#### Scenario: An entry without a cover

- **WHEN** no cover image could be derived for an entry
- **THEN** the list response carries no cover image URL for it and the response stays valid
