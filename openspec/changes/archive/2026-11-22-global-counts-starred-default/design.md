## Context

- Sidebar counts today come from the `unread_count` fields on the `FeedDto`/`FolderDto` returned by `list_feeds`/`list_folders`; the two global rows ("All entries", "★ Starred") are rendered badge-less in `src/components/Sidebar.vue` and no backend data for them exists.
- `src/App.vue` keeps a single `view: "all" | "unread"` ref shared by every selection. `reload()` applies it only when the selection is global (`all` or `starred`), so the Starred view currently inherits whichever tab was active before — the behavior this change removes.
- Counts refresh through `refreshCounts()` (`listFeeds` + `listFolders`), called from `reload()` and `applyRead()`. `toggleStar()` does not call it today, so starring/unstarring would leave a new badge stale.
- Constraint: no schema change, no new dependency, and the existing feed/folder badge behavior must not regress. See proposal.md for motivation and specs/ for the resulting contract.

## Goals / Non-Goals

**Goals:**

- One cheap backend command that returns both global counts (total stored entries, total starred entries).
- Badges on the two global sidebar rows, refreshed on every path that can change the counts.
- The Starred view always lists all starred entries and never offers the Unread tab, without disturbing the All/Unread tab on "All entries".

**Non-Goals:**

- Unread-based counts for the global views (badges show totals, not unread counts).
- Any change to feed/folder badges, folder aggregation, or the existing unread badge style.
- Fixing the pre-existing quirk where a feed/folder selection keeps the "Unread" tab highlighted while listing the full scope list (out of scope; untouched).
- Schema/migration work, new dependencies, or renaming the shared `view` state.

## Decisions

**1. Single `entry_counts command returning both counts.**
A new Tauri command runs one statement — `SELECT COUNT(*), COALESCE(SUM(is_starred), 0) FROM entries` — returning `{ all_entries, starred_entries }`. Alternatives considered: counting client-side from `list_entries` (would ship every row's `content_html` over IPC just to count — rejected); adding fields to `list_feeds`/`list_folders` (wrong DTOs, neither covers the global rows — rejected); two separate commands (two IPC round trips for data that one scan yields — rejected).

**2. Badges reuse the existing pill style and hide at zero.**
Both global badges render with the existing `.unread-badge` class and the same `v-if="count > 0"` convention as feed/folder rows, so the sidebar stays visually consistent and an empty state looks unchanged. No new CSS.

**3. Forcing "All" in the Starred view is done at load time, not by resetting shared state.**
`reload()` computes the effective view: `starred → "all"`, other global selections → `view`, feed/folder → `"all"` (unchanged). `EntryList` receives `"all"` as its active tab whenever the Starred view is selected. The alternative — setting `view.value = "all"` when Starred is selected — was rejected because it would silently clear the user's Unread tab on "All entries" when they come back. `setView` additionally ignores an `unread` request while the Starred view is selected (defense in depth; the tab is hidden anyway).

**4. The Unread tab is hidden, not disabled, while the Starred view is selected.**
`EntryList` gains a boolean prop (e.g. `unreadFilterAvailable`, false only for the starred selection) gating the Unread button; the All tab stays visible as the active indicator. A permanently disabled control that can never be enabled adds noise, and the spec says the filter is not offered.

**5. Counts are re-fetched, not patched locally.**
`refreshCounts()` also fetches the two global counts (via `Promise.all` with the existing calls), and `toggleStar()` now awaits it in addition to the existing `reload()`/`applyRead()` paths. Local increment/decrement was rejected: starring happens from any view, and drift between a hand-patched badge and the DB is worse than one extra `COUNT(*)` on a local table.

## Risks / Trade-offs

- [Extra `COUNT(*)` over `entries` on every count refresh] → single local scan alongside queries that already run on the same paths; acceptable at this scale, and the two counts can be folded into an existing query later if profiling ever shows it.
- [Behavior change to the Starred view's filter] → deliberate and user-approved; carried as a REMOVED + ADDED pair in the `entry-starring` delta so the archive records the replacement explicitly.
- [Stale badge if a mutation path forgets to refresh] → every existing mutation path already funnels through `reload()` or `refreshCounts()`; the only gap (`toggleStar`) is closed in this change.
- [Hiding the Unread tab changes the entry-list header layout for the starred selection] → only one button disappears; no other layout rules depend on the tab count.

## Migration Plan

None: no schema, stored data, or API compatibility to migrate. The only externally visible break is the Starred view's filter behavior, specified in the `entry-starring` delta.

## Open Questions

None.
