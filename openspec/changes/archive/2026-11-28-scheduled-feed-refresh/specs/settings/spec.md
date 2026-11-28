## ADDED Requirements

### Requirement: Settings dialog hosts the scheduled refresh controls

The Settings dialog SHALL contain a refresh section offering the schedule's switch, its interval, the time of the last scheduled refresh, and when the next one is due. Editing either control SHALL persist the choice immediately and SHALL report the new setting inside the dialog, and the section SHALL state plainly whether the schedule is on or off.

#### Scenario: Turning the schedule on from Settings

- **WHEN** the reader enables the schedule in the Settings dialog and picks a thirty-minute interval
- **THEN** the dialog reports that the schedule is on with that interval and the choice survives a restart

#### Scenario: The dialog reports the last and next refresh

- **WHEN** the reader opens the Settings dialog after a scheduled refresh ran
- **THEN** the refresh section shows when it ran and when the next one is due

#### Scenario: Turning the schedule off

- **WHEN** the reader switches the schedule off
- **THEN** the dialog reports that it is off and no further scheduled refresh runs

#### Scenario: Interval covers the offered durations

- **WHEN** the reader opens the interval control
- **THEN** it offers a fixed set of durations and does not require typing a number
