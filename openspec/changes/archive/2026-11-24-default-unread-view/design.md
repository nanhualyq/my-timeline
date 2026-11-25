## Context

See proposal.md — Why. The list filter is already one piece of state in `App.vue` (`view`) plus one derived function (`activeView()`), but two lines make it global-only: `activeView()` returns `"all"` for any non-global selection, and `setView()` reassigns the selection to "All entries" when a feed or folder is selected. `EntryList.vue` already renders the tabs, hides the Unread tab when `unreadFilterAvailable` is false (the Starred view), and emits `setView`; it needs no change.

## Goals / Non-Goals

**Goals:**

- One filter state, applied at whatever scope is selected, defaulting to unread.
- No change to the paging contract, the batch actions, or the sidebar counts: they all consume `activeView()` already.

**Non-Goals:**

- Persisting the chosen filter across launches.
- Per-scope filters (a different tab remembered for each feed).

## Decisions

### The filter is a single app-wide state, not one per selection

`view` starts as `"unread"` and stays whatever the reader last chose; selecting a different feed or folder applies the same filter. The alternative — resetting to unread on every selection — makes the All tab unusable for browsing a folder's read archive, because every hop between feeds would drop back to unread.

Scenario "The filter survives a change of selection" pins this down.

### Starred remains the one unfiltered list

`activeView()` still short-circuits to `"all"` for the Starred view, and `setView()` still ignores tab clicks there (the list shows the All tab alone, so there is no unread filter to click). This is not a default but a rule: the Starred list is the only place where read and unread entries are always mixed, which is what it is for.

### Batch actions keep their existing availability rule

`bulk-mark-read` offers its in-list actions for a feed, a folder, and the unread view of "All entries". That rule is written in terms of the selection, not the tab, so a feed's list keeps both actions whichever filter is showing — and the filter being unread by default now just means those actions usually act on an all-unread page. No spec change is needed there.

### The empty state stays one message

A list filtered to unread that has nothing left renders the existing "No entries." line. Adding a second word for "no unread entries" would mean threading the filter into a component that currently only knows what rows it was handed.

## Risks / Trade-offs

- [A reader who wants the archive sees less by default] → one click on the All tab, whose position is where it already was, and the choice carries across selections.
- [Totals change meaning at launch: the progress line now counts unread entries] → that is the intended reading ("12 of 80 unread"), and the sidebar badges already describe unread separately.
- [Batch marking in a feed's unread list removes every loaded row and refills the page] → already handled by the existing restore path, which the specs cover.
- [The in-flight batch guard could mis-associate rows when the filter changes mid-load] → `setView()` already bumps the generation counter through `startOver()`, so stale batches are discarded.

## Migration Plan

No schema, data, or API change. Rollback is the initial value of `view` and the two removed lines in `activeView()` / `setView()`.

## Open Questions

None.
