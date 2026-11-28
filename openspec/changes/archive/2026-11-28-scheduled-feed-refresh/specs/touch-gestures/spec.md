## ADDED Requirements

### Requirement: A downward pull at the top of the list refreshes the current scope

On a touch layout, a downward drag that begins while the entry list is scrolled to its top SHALL become a pull: the list SHALL follow the finger with resistance up to a limit, and releasing past the commit threshold SHALL refresh the scope the list is currently showing — the selected feed, the selected folder, the starred entries' feeds, or every feed when the list is showing all entries. Releasing before the threshold SHALL return the list to rest and refresh nothing. The pull SHALL report itself through the same refresh panel as every other trigger, so it needs no indicator of its own.

#### Scenario: Pulling the list refreshes what it shows

- **WHEN** the reader pulls down at the top of a list showing one feed's entries
- **THEN** that feed is fetched and the panel reports the run

#### Scenario: Pulling the all-entries list refreshes every feed

- **WHEN** the reader pulls down at the top of the list showing all entries
- **THEN** every feed is fetched and the panel reports the run

#### Scenario: A short pull refreshes nothing

- **WHEN** the reader drags the list down without passing the commit threshold
- **THEN** the list returns to its resting position and no feed is fetched

#### Scenario: A pull does not restart a run in flight

- **WHEN** the reader pulls while a run is already fetching the same feeds
- **THEN** no second run starts and the in-flight run's summary covers those feeds once

### Requirement: The pull is claimed only where it is a pull

The pull SHALL be recognized only for a downward movement that begins with the entry list already at its top, and SHALL NOT be claimed for a movement that begins while the list is scrolled, inside the reading pane, or during a stationary hold that the row menu owns. A movement the pull does not claim SHALL stay available to the browser for scrolling and to the row gestures.

#### Scenario: A downward drag mid-list scrolls

- **WHEN** the reader drags downwards while the list is scrolled away from its top
- **THEN** the list scrolls and no refresh is triggered

#### Scenario: The reading pane never pulls to refresh

- **WHEN** the reader drags downwards inside an article
- **THEN** the article scrolls and no refresh is triggered

#### Scenario: A hold is still the row menu

- **WHEN** the reader presses a row at the top of the list and holds without moving
- **THEN** the row menu opens and no refresh is triggered

#### Scenario: A horizontal drag at the top is still a row action

- **WHEN** the reader drags a row at the top of the list horizontally
- **THEN** the row action commits and no refresh is triggered

### Requirement: Refresh stays reachable without the pull

Because the one-pane layout no longer shows a refresh button, the refresh actions SHALL remain reachable from the sidebar's feed and folder menus, and the scheduled refresh SHALL keep the library current on its own, so the pull is never the only way to fetch.

#### Scenario: Refreshing a feed without the pull

- **WHEN** a reader who does not know about the pull opens a feed's menu in the sidebar
- **THEN** a refresh action is offered there and running it reports through the panel

## MODIFIED Requirements

### Requirement: A gesture claims the drag only after locking its direction

A horizontal gesture on a row SHALL take over the drag only once the movement has locked its direction as horizontal; until then the movement SHALL remain available to the list for vertical scrolling, except at the top of the entry list, where a downward movement past the movement slop belongs to the pull that refreshes the current scope. A touch that moves less than the gesture's movement slop SHALL be treated as a tap and SHALL open the entry as a tap does.

#### Scenario: Scrolling a row still scrolls

- **WHEN** the reader drags a row vertically while the list is scrolled away from its top
- **THEN** the list scrolls and no row action is committed

#### Scenario: A slightly diagonal downward drag scrolls

- **WHEN** the reader drags a row downwards with a small horizontal component while the list is scrolled away from its top
- **THEN** the list scrolls and the row action is not committed

#### Scenario: A tap still opens the entry

- **WHEN** the reader touches a row and releases it without moving beyond the movement slop
- **THEN** the entry opens exactly as a tap opens it

#### Scenario: A downward movement at the top is not scrolling

- **WHEN** the reader drags downwards past the movement slop while the list is at its top
- **THEN** the movement becomes a pull and starts no row action
