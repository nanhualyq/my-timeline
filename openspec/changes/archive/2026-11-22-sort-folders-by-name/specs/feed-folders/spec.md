## ADDED Requirements

### Requirement: Folders are ordered by name

The system SHALL display folders ordered by their names using default string comparison (case-sensitive, code-unit order), and SHALL NOT provide any way for the user to change folder order.

#### Scenario: Folders appear in name order regardless of creation order

- **WHEN** the user creates a folder named "Tech" and then a folder named "Books"
- **THEN** the sidebar lists "Books" before "Tech"

#### Scenario: Default string comparison is case-sensitive

- **WHEN** folders named "apple" and "Banana" exist
- **THEN** the sidebar lists "Banana" before "apple", because uppercase letters sort before lowercase ones under default string comparison

#### Scenario: Renaming changes a folder's position in the list

- **WHEN** the user renames a folder so its name sorts earlier than before
- **THEN** the sidebar updates and the folder appears at the position implied by its new name

#### Scenario: No reorder controls are shown

- **WHEN** the sidebar displays a folder row
- **THEN** the row offers no control for moving the folder up or down

## REMOVED Requirements

### Requirement: User can reorder folders

**Reason**: Manual folder ordering added sidebar clutter and extra backend surface (a command, a DB function, and a `position` column) for little benefit; folders are now always sorted by name.

**Migration**: No user action required. The move up/down controls disappear and any previously chosen folder order is ignored; folders are listed in name order instead.
