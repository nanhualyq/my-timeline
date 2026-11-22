## ADDED Requirements

### Requirement: Sidebar shows the total entry count on All entries
The system SHALL show a count badge on the "All entries" row in the sidebar containing the total number of stored entries, read and unread alike. The badge SHALL be hidden when no entry is stored and SHALL stay current as entries arrive or are removed.

#### Scenario: Count reflects stored entries

- **WHEN** the feeds hold 40 stored entries in total and the sidebar is displayed
- **THEN** the "All entries" row shows a badge with the number 40

#### Scenario: Count includes read entries

- **WHEN** every stored entry has been marked read
- **THEN** the "All entries" badge still shows the total number of stored entries

#### Scenario: Badge hidden when there are no entries

- **WHEN** no entry is stored
- **THEN** no badge is shown on the "All entries" row

#### Scenario: Count updates after a refresh stores new entries

- **WHEN** a refresh stores five new entries
- **THEN** the "All entries" badge increases by five
