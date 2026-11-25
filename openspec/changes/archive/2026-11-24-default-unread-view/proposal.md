## Why

Every list opens on "All" — including a freshly read feed whose only interesting content is what is still unread. A reader has to switch to the Unread tab on arrival, and inside a feed or folder they cannot switch at all: the tab jumps back to "All entries" instead of filtering the selection. The natural reading order for an RSS queue is unread first, with the archive one click away.

## What Changes

- Every list — a feed, a folder, and "All entries" — opens on the unread filter.
- The All / Unread tabs apply to the current selection: choosing Unread inside a feed filters that feed instead of leaving for "All entries".
- The Starred view keeps showing every starred entry, read or unread, with no filter offered.
- Not included: per-selection remembered filters beyond the current session, and any new filter (e.g. starred-only, per-feed unread counts in tabs).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `rss-reader`: "User can browse and filter entries" gains the default filter, the per-selection scope of the filter, and the Starred exception.

## Impact

- `src/App.vue`: initial view becomes `unread`; `activeView()` stops forcing `all` outside the global views; `setView()` stops redirecting feed and folder selections to "All entries".
- `src/components/EntryList.vue`: no behavior change — the tabs already emit `setView` and hide the Unread tab where `unreadFilterAvailable` is false.
- `src/__tests__/app.spec.ts`: the paging and scope specs that rely on the old default move to an explicit All tab, plus new specs for the default and for filtering inside a feed, a folder, and Starred.
