# rss-reader Specification

## Purpose

Lets users subscribe to RSS/Atom feeds, fetch their entries into local storage, and read them with read/unread tracking — the minimal reading loop of the RSS reader.

## Requirements

### Requirement: User can add a feed by URL

The system SHALL accept a URL from the user and, when the URL points directly to an RSS or Atom feed, create a subscription for it. When the URL points to a web page rather than a feed, the system SHALL attempt to discover a feed linked from that page and subscribe to it. The system SHALL reject the input with a visible error when neither yields a valid feed.

#### Scenario: Adding a direct feed URL

- **WHEN** the user submits a URL that returns a valid RSS or Atom feed
- **THEN** a new feed appears in the sidebar with its title and zero or more entries stored

#### Scenario: Adding a page URL with a discoverable feed

- **WHEN** the user submits a URL that returns an HTML page containing a link to a feed
- **THEN** the system subscribes to the linked feed and it appears in the sidebar

#### Scenario: Adding an invalid URL

- **WHEN** the user submits a URL that is neither a valid feed nor a page linking to one
- **THEN** no feed is created and the user sees an error message

### Requirement: User can remove a feed

The system SHALL allow the user to remove a subscription, and removal SHALL also remove its stored entries.

#### Scenario: Removing a feed

- **WHEN** the user removes a feed from the sidebar
- **THEN** the feed and all its entries disappear from the application and do not reappear after a restart

### Requirement: User can refresh all feeds manually

The system SHALL provide a manual action that fetches all subscribed feeds concurrently and stores new or updated entries. The refresh SHALL complete in a bounded time with per-request timeouts.

#### Scenario: Refreshing all feeds

- **WHEN** the user triggers a full refresh and the feeds respond successfully
- **THEN** new entries appear in the entry list ordered by publication time and each feed's last-fetched timestamp is updated

### Requirement: User can refresh a single feed manually

The system SHALL provide a manual per-feed refresh action that fetches only the selected feed and stores its entries.

#### Scenario: Refreshing one feed

- **WHEN** the user triggers refresh on one feed
- **THEN** only that feed is fetched and its entries are updated; other feeds' data is unchanged

### Requirement: Feed failures are isolated and visible

A feed that fails to fetch or parse SHALL record a visible error on that feed without blocking or failing the refresh of other feeds. A subsequent successful refresh SHALL clear the recorded error.

#### Scenario: One feed fails during a full refresh

- **WHEN** a full refresh runs and one feed's request fails while others succeed
- **THEN** the failing feed shows an error indicator, the other feeds' entries are stored, and the refresh action completes

#### Scenario: A failing feed recovers

- **WHEN** a feed that previously failed is refreshed and succeeds
- **THEN** its entries are stored and its error indicator is cleared

### Requirement: Entries are deduplicated per feed

The system SHALL store each entry at most once per feed, using the entry's GUID as the identity and falling back to its link when no GUID is present. Re-fetching a feed SHALL NOT create duplicate entries; updated entry content SHALL update the existing record.

#### Scenario: Re-fetching an unchanged feed

- **WHEN** a feed is refreshed and contains entries already stored
- **THEN** no duplicate entries are created

#### Scenario: Entries without GUIDs are identified by link

- **WHEN** a feed entry has no GUID but has a link that was already stored for that feed
- **THEN** the entry is not stored again

### Requirement: User can browse and filter entries

The system SHALL list stored entries newest first and offer at least three views: all entries, unread only, and entries of a single selected feed. Selecting an entry SHALL display its content in the reading pane.

#### Scenario: Viewing unread entries

- **WHEN** the user selects the unread view
- **THEN** only entries not yet marked read are listed, newest first

#### Scenario: Viewing a single feed's entries

- **WHEN** the user selects a feed in the sidebar
- **THEN** only that feed's entries are listed

### Requirement: User can toggle read state

The system SHALL let the user mark an entry as read or unread, persist the state across restarts, and reflect it in the unread counts and unread filter.

#### Scenario: Marking an entry read

- **WHEN** the user marks an unread entry as read
- **THEN** the entry leaves the unread view, its feed's unread count decreases, and the state survives an application restart

#### Scenario: Marking an entry unread

- **WHEN** the user marks a read entry as unread
- **THEN** the entry reappears in the unread view with an incremented unread count

### Requirement: Entry content is sanitized before rendering

The system SHALL sanitize entry HTML in the Rust backend before it reaches the webview, stripping scripts and other active content, so that rendering an entry cannot execute attacker-supplied code.

#### Scenario: Rendering an entry containing a script tag

- **WHEN** an entry whose stored HTML contains a script tag is opened in the reading pane
- **THEN** the script does not execute and the rest of the entry content renders

### Requirement: Reader UI presents three panes

The application SHALL present a three-pane layout: a feed sidebar with unread counts and per-feed refresh controls, an entry list with a refresh-all action, and a reading pane showing the selected entry with a read/unread toggle.

#### Scenario: Layout visible on launch

- **WHEN** the application window opens after a successful startup
- **THEN** the sidebar, entry list, and reading pane are all visible and navigable
