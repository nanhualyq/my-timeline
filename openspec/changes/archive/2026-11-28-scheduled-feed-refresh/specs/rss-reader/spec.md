## ADDED Requirements

### Requirement: User can refresh a folder

The system SHALL provide a refresh action for a folder that fetches only the feeds assigned to it, and SHALL report the outcome of each feed in the refresh panel rather than in a dialog the reader has to dismiss.

#### Scenario: Refreshing a folder

- **WHEN** the user refreshes a folder holding four feeds
- **THEN** only those four feeds are fetched, the panel reports each result, and feeds outside the folder are unchanged

#### Scenario: A folder refresh does not block the application

- **WHEN** a folder refresh runs
- **THEN** the reader can still open entries and change the selection while it proceeds

## MODIFIED Requirements

### Requirement: User can refresh all feeds manually

The system SHALL provide a manual action that fetches all subscribed feeds and stores new or updated entries, and the run SHALL complete in a bounded time with per-request timeouts. The run SHALL report its progress while it is in flight and SHALL end with a summary stating how many feeds were fetched, how many entries were added, and which feeds failed, so that a run with failures is never silent.

#### Scenario: Refreshing all feeds

- **WHEN** the user triggers a full refresh and the feeds respond successfully
- **THEN** new entries appear in the entry list ordered by publication time, each feed's last-fetched timestamp is updated, and the summary reports the entries added

#### Scenario: A full refresh is not silent

- **WHEN** the user triggers a full refresh
- **THEN** the panel shows progress before the refresh finishes and a summary after it does

#### Scenario: Partial failure is reported

- **WHEN** a full refresh runs and some feeds fail while others succeed
- **THEN** the successful feeds' entries are stored and the summary names the feeds that failed

### Requirement: User can refresh a single feed manually

The system SHALL provide a manual per-feed refresh action that fetches only the selected feed and stores its entries. The run SHALL be the same kind of run as a full refresh, reported and summarized the same way, and it SHALL NOT refetch feeds that a run already in flight has fetched.

#### Scenario: Refreshing one feed

- **WHEN** the user triggers refresh on one feed
- **THEN** only that feed is fetched and its entries are updated; other feeds' data is unchanged

#### Scenario: A per-feed refresh is reported

- **WHEN** the user refreshes one feed that fails
- **THEN** the panel names that feed and shows the reason it failed

### Requirement: Reader UI presents three panes

The application SHALL present a three-pane layout: a feed sidebar with unread counts and per-feed refresh actions, an entry list, and a reading pane showing the selected entry with a read/unread toggle. On a layout wide enough for the three panes the entry list SHALL offer a refresh action as a control in its header; on the one-pane layout that control SHALL NOT be shown and a downward pull at the top of the entry list SHALL refresh the current list's scope instead.

#### Scenario: Layout visible on launch

- **WHEN** the application window opens after a successful startup
- **THEN** the sidebar, entry list, and reading pane are all visible and navigable

#### Scenario: The wide layout offers a refresh control

- **WHEN** the application is displayed in a layout wide enough for the three panes
- **THEN** the entry list's header shows a refresh action

#### Scenario: The one-pane layout offers no refresh button

- **WHEN** the application is displayed on the one-pane layout
- **THEN** the entry list's header shows no refresh button and a downward pull at the top of the list refreshes the current scope
