## Purpose

Keeps the local library current without the reader asking, and makes every fetch legible while it runs: a schedule the reader sets, a single shared refresh runner behind every trigger, polite per-host request pacing, live per-feed progress in a floating panel, and a summary that says what the run actually added.

## ADDED Requirements

### Requirement: The reader can schedule refreshes

The system SHALL offer a refresh schedule consisting of a switch and an interval, and SHALL keep the chosen values across application restarts. The schedule SHALL be stored with the device's own library and SHALL NOT be carried into or read from the sync document, so two installations may run different schedules. Intervals SHALL be chosen from a set of fixed durations rather than entered as an arbitrary number.

#### Scenario: Turning the schedule on

- **WHEN** the reader enables the refresh schedule and chooses an interval
- **THEN** the application begins refreshing feeds on that interval without any further action

#### Scenario: The choice survives a restart

- **WHEN** the reader sets an interval, closes the application, and starts it again
- **THEN** the schedule is still enabled with the same interval

#### Scenario: The schedule is not shared

- **WHEN** an installation with its own schedule syncs with another installation
- **THEN** neither installation's schedule changes

#### Scenario: The schedule is off by default

- **WHEN** the application is started for the first time
- **THEN** no scheduled refresh runs until the reader enables it

### Requirement: Only feeds whose content is older than the interval are fetched

A scheduled check SHALL fetch a feed only when the interval has passed since that feed was last fetched, counting any fetch — scheduled, manual, or per-feed — as a fetch. A feed fetched moments ago by hand therefore SHALL NOT be fetched again by a check that follows. When the schedule is first enabled, every feed SHALL be eligible at once.

#### Scenario: A recently fetched feed is skipped

- **WHEN** a scheduled check runs two minutes after the reader refreshed one feed by hand, with the interval set to thirty minutes
- **THEN** that feed is not fetched and the other eligible feeds are

#### Scenario: Nothing is eligible

- **WHEN** a scheduled check runs and no feed is older than the interval
- **THEN** no feed is fetched and no run is reported to the reader

#### Scenario: The first check after enabling fetches everything

- **WHEN** the reader enables the schedule on a library whose feeds were last fetched long ago
- **THEN** every feed is fetched by the following check

### Requirement: The schedule runs only while the application is visible

A scheduled check SHALL NOT fetch anything while the application is hidden or in the background, and SHALL run a check when the application becomes visible again, so a returning reader sees a library brought up to date without the application fetching while nobody is watching.

#### Scenario: No fetching while hidden

- **WHEN** the application is in the background and an interval elapses
- **THEN** no feed is fetched until the application is visible again

#### Scenario: Returning catches up

- **WHEN** the application becomes visible after being hidden beyond the interval
- **THEN** the feeds that became eligible are fetched

### Requirement: One refresh runs at a time and no feed is fetched twice

Every trigger — the list's refresh action, its keyboard shortcut, the pull gesture, the schedule, and returning to the foreground — SHALL be served by one shared run. While a run is in flight, a further trigger SHALL NOT start a second concurrent run and SHALL NOT fetch a feed the in-flight run has already dispatched or completed. A trigger asking for feeds the in-flight run does not cover SHALL add those feeds to it.

#### Scenario: A second trigger during a run adds nothing

- **WHEN** the reader activates the refresh action twice in quick succession
- **THEN** each feed is fetched once and one summary is reported

#### Scenario: A narrower trigger during a full run

- **WHEN** a full run is in flight and the reader pulls to refresh the feed they have selected
- **THEN** that feed is not fetched a second time and the run's summary covers it once

#### Scenario: A wider trigger during a run adds feeds

- **WHEN** a run for one folder is in flight and the reader triggers a refresh of all feeds
- **THEN** the feeds outside that folder are added to the same run

### Requirement: Feeds on the same host are fetched one at a time

A run SHALL fetch feeds from different hosts in parallel, bounded by a concurrency limit, and SHALL NEVER have more than one request in flight to the same host at a time. Two consecutive requests to the same host SHALL be separated by a delay, and the run's dispatch order SHALL spread its first requests across the distinct hosts it has targets for rather than exhausting one host first. Feeds are grouped by hostname for this purpose.

#### Scenario: Two feeds on one host do not overlap

- **WHEN** a run fetches two feeds whose URLs share a hostname, and both are eligible
- **THEN** the second request starts only after the first has finished

#### Scenario: Many feeds on one host do not crowd out the others

- **WHEN** a run's targets are mostly on one host with a few on other hosts
- **THEN** the feeds on other hosts are fetched concurrently with that host's first feed, not queued behind its whole queue

#### Scenario: Different hosts run in parallel

- **WHEN** a run's targets are spread across several hosts
- **THEN** several requests are in flight at the same time, up to the concurrency limit

### Requirement: A host that refuses requests is backed off

When a host answers a fetch with a rate-limit or temporary-unavailable status, the run SHALL hold that host's remaining feeds and retry them after a delay that grows with each refusal, while continuing with every other host. The affected feeds SHALL be reported as deferred for that host rather than as individually failed, and a retry that succeeds SHALL leave no error recorded on those feeds.

#### Scenario: Rate limiting a host does not fail its feeds

- **WHEN** a run fetches four feeds on one host and the first answers with a rate-limit status
- **THEN** the remaining three are deferred rather than reported as failed, and the other hosts' feeds continue

#### Scenario: The deferred feeds are retried

- **WHEN** the delay for a backed-off host has passed and the run is still in flight
- **THEN** that host's deferred feeds are fetched and a success clears any recorded error

#### Scenario: A second refusal waits longer

- **WHEN** the retried request is refused again
- **THEN** the next retry waits longer than the previous one did

### Requirement: A run reports its progress while it is in flight

A run SHALL report, as it proceeds, how many feeds it has finished out of how many it is fetching and the outcome of each feed. The application SHALL present this in a floating panel that shows the completed count, the outcome of the feeds finished so far, and which feed is failing and why. The panel SHALL NOT block interaction with the rest of the application, SHALL NOT be a modal dialog, and SHALL NOT appear for a run that finishes before the reader could notice it.

#### Scenario: Progress is visible during a long run

- **WHEN** a run fetches twenty feeds and takes several seconds
- **THEN** the panel shows the completed count rising before the run ends

#### Scenario: A failing feed names its reason

- **WHEN** a feed fails because the server answered with an error status
- **THEN** the panel names that feed and shows the reason it failed

#### Scenario: The reader keeps working during a run

- **WHEN** a run is in flight and the reader opens an entry or changes the selection
- **THEN** the application responds and the run continues

#### Scenario: A short run does not flash a panel

- **WHEN** a run finishes almost immediately
- **THEN** no panel is presented for it, and its summary is not lost

### Requirement: A run ends with a summary the reader can act on

When a run finishes the panel SHALL report how many feeds were fetched, how many entries were added, and how many feeds failed. That summary SHALL withdraw on its own when nothing failed and SHALL remain, with a way to retry the failed feeds and a way to dismiss it, when anything did. A retry SHALL fetch only the failed feeds.

#### Scenario: A clean run summarizes and withdraws

- **WHEN** a run fetches every feed successfully
- **THEN** the summary reports the feed count and the number of entries added, then withdraws without the reader dismissing it

#### Scenario: A run with failures stays

- **WHEN** a run finishes with two failed feeds
- **THEN** the summary stays, both failures are listed, and a retry action is offered

#### Scenario: Retrying fetches only what failed

- **WHEN** the reader activates the retry action on a summary listing two failed feeds
- **THEN** only those two feeds are fetched

#### Scenario: Newly added entries are counted

- **WHEN** a run stores twelve entries that the library did not hold before
- **THEN** the summary reports twelve added entries

### Requirement: A run can be stopped

While a run is in flight the reader SHALL be able to stop it. Stopping SHALL prevent further feeds from being fetched, SHALL keep the results already recorded, and SHALL leave the run's summary reporting that it was stopped early and how many feeds it completed.

#### Scenario: Stopping a long run

- **WHEN** the reader stops a run partway through
- **THEN** no further feeds are fetched, the completed feeds keep their results, and the summary says it was stopped

#### Scenario: Stopping does not undo a stored feed

- **WHEN** a feed was fetched successfully before the run was stopped
- **THEN** its entries remain stored and its error state stays cleared

### Requirement: The refresh panel does not collide with the undo offer

The refresh panel and the undo offer that follows a batch mark SHALL be presented in one stacked region rather than overlapping each other, and both SHALL sit below any dialog the reader has open. The panel's content SHALL be announced to assistive technology without moving focus out of the reader's current control.

#### Scenario: Both notices at once

- **WHEN** a batch mark shows its undo offer while a run is reporting progress
- **THEN** both notices are readable at the same time and neither covers the other

#### Scenario: A dialog stays on top

- **WHEN** the reader opens a dialog while the panel is showing
- **THEN** the dialog is above the panel and usable

#### Scenario: Progress is announced without stealing focus

- **WHEN** the panel reports progress while the reader is typing in a field
- **THEN** the announcement does not move focus out of that field
