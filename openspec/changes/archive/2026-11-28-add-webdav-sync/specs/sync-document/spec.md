## ADDED Requirements

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
