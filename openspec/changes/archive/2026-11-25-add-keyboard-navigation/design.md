## Context

See proposal.md for motivation. The current state that shapes the approach:

- No keyboard handling exists beyond `Escape`. `SettingsDialog.vue`, `FeedEditDialog.vue`, and `RowMenu.vue` each install their own `document` keydown listener to close themselves. Any global binding must not fight three scattered listeners.
- Entry rows are plain `<li>` elements with a click handler. They are not focusable and hold no focus state, so "keyboard navigation" has no existing DOM focus concept to build on.
- `App.vue` already owns every piece of list state that a cursor needs: `entries` (the loaded window), `selectedEntryId` (the opened row), and `selectEntry()`, which fetches content and marks the row read. `EntryList.vue` is presentational apart from the `IntersectionObserver` that asks for the next batch.
- Selecting an entry is already a single-entry mark-read path that patches the row in place without resetting the list, whereas the batch path (`applyBatchRead`) intentionally resets an unread list. See `openspec/specs/bulk-mark-read/spec.md` — "The list is restored after a batch mark".
- Every row is exactly `7.5rem` tall (`EntryList.vue`), so list geometry is uniform, but it is a CSS value, not something JavaScript currently knows.
- The list only ever appends downwards; a cursor that walks off the end needs the same `loadMore()` the scroll sentinel uses.

## Goals / Non-Goals

**Goals:**

- One owner for keyboard state: a single listener, a single resolver, and a single source of truth for the bindings that both dispatch and the help overlay read.
- Keystroke-to-cursor feedback with no network in the path; persistence coalesced per burst.
- Cursor behaviour that is defined for the ugly cases: reloads, refreshes, deleted entries, list reset, empty list, input focus, open dialogs.
- Binding rules testable without mounting a component.

**Non-Goals:**

- No focus model between panes, no sidebar cursor, no visual mode, no registers, no macros, no user-remappable keys.
- No new backend commands. `markReadMany`, `setRead`, `setStarred`, `getEntry`, `refreshAll` already cover every action.
- Not making the rows keyboard-focusable in the DOM, and not providing a `aria-activedescendant` style screen-reader cursor in this change (see Risks).

## Decisions

### The cursor is `selectedEntryId`, not new state

Because moving the cursor opens the entry, the cursor row and the open row are the same row by definition. The cursor is therefore the existing `selectedEntryId`; its position is `entries.findIndex(e => e.id === selectedEntryId)`.

*Alternative considered:* a separate `cursorIndex` next to `selectedEntryId`. Rejected — it creates two states that must be kept in step through every reload path (`reload`, `startOver`, `loadMore`, `applyBatchRead`) and admits the "cursor on row 5, pane showing row 9" inconsistency. The one thing a bare index would give us — a position to clamp to when the open entry disappears — is cheap to recover: keep a `lastCursorIndex` written on every move, used only as the fallback when the id is no longer in `entries`.

The identity of the cursor is also what keeps the removal case simple. `reload` is the only path that learns an entry no longer exists (it asks for it directly), so `reload` is where the fallback lives: `trackCursor` keeps the recorded position current, `replaceVanishedCursor` stands in when the entry is gone. `doRemove` keeps its own explicit clearing for the feed the reader removed — removing a subscription takes the reader out of the entry instead of silently swapping in a neighbour, which is what the application already did and what `entry-list-pagination` already documents.

### Bindings are data; the resolver and the help overlay both read it

A single table describes every binding as `{ keys, scope, description }` where `keys` is a sequence like `["j"]`, `["g", "g"]`, or `["ctrl", "d"]`. The dispatcher looks up the resolved sequence in this table and the `?` overlay renders the same table. This is what makes the spec's "the reference matches the bindings" scenario hold by construction rather than by discipline.

*Alternative considered:* a chain of `switch`es with the overlay's text written separately. Rejected — the two drift the first time a key is rebound.

### A pure resolver module, one document listener

`src/lib/keymap.ts` exposes a resolver that takes the incoming keystroke plus the pending prefix and count, and returns one of: `{ kind: "pending" }`, `{ kind: "action", action, count }`, or `{ kind: "ignored" }`. It holds no timers and no DOM access. `App.vue` installs the one `document` keydown listener, keeps the pending `{ prefix, count }` state, and runs the resolver.

Rejected alternatives:

- *Per-component listeners.* This is what the codebase does today for `Escape` and it is exactly the shape that breaks under a `g` prefix: two components cannot share "a `g` was pressed".
- *A hotkey library* (hotkeys-js, tinykeys). They model chords, not Vim's count-plus-motion grammar, and pulling a dependency in for ~15 bindings plus a custom prefix node is not worth the supply-chain and bundle cost.
- *CSS `:focus` plus `tabindex`.* Would give keyboard access for free but not counts, prefixes, or bursts, and would leave a focus ring on every row.

Where the resolver returns `ignored`, the container calls nothing and does not `preventDefault`, so the webview's own handling survives.

### The burst is the debounce boundary

A move updates `selectedEntryId` and optimistically patches the row (read dot off, already handled by `patchEntry`) synchronously. Then:

- the landed row id is stored as `pendingOpenId`
- every row passed and not yet read is added to `pendingReadIds` (a `Set`)
- a flush timer is armed, resetting on every further move

On flush: one `getEntry(pendingOpenId)` for content, one `markReadMany([...pendingReadIds])` for the whole burst, one `refreshCounts()`. The existing guard in `selectEntry` (`if (selectedEntryId.value !== item.id) return`) already prevents a slow response from painting an entry the cursor has left; the burst path keeps that guard.

*Alternative considered:* debounce only the content fetch and write read state per step. Rejected — `applyRead` calls `refreshCounts()`, which issues `listFeeds`, `listFolders`, and `entryCounts`. Per repeat that is a 150 IPC/s storm for no benefit, since intermediate counts are never observed.

Flush is also forced — not just timed — when the list is about to be replaced (`select`, `setView`, `startOver`), on window blur, and on unmount, so a pending mark cannot be silently dropped (spec: "Pending marks are not lost when the list is replaced").

Flush delay: 120ms. It is below a typical OS key-repeat interval, so releasing the key lands the write immediately, and it is above the inter-keystroke gap of a normal burst.

### `g` never blocks; only its resolution is deferred

`g` is a pending prefix, not a modal state. Nothing is delayed for other keys — `g` followed by `j` resolves the prefix as unknown and then the `j` still moves the cursor. A 1000ms inactivity timer discards the prefix and, importantly, does *not* reinterpret the `g` as a bare command, because there is no bare `g` binding. `gg` therefore has no perceptible latency: the first `g` costs nothing, and the second resolves the pair.

The count prefix is a separate pending field. Pressing digits accumulates the count; the next resolved action consumes it, and a resolved non-motion clears it. The spec pins one deliberate oddity: `3gg` goes to the first row, because a count has no meaning for an absolute jump to the top.

### Cursor-driven marking needs a non-resetting mark path

`applyBatchRead` resets the list when the active view is `unread`. Reusing it for burst marking would reset `entries` (`startOver()` clears the array, bumps `generation`, refetches) on every burst while the cursor sits on a row — the list would churn under the reader, breaking the spec's "does not reflow the list".

So burst marking gets its own path: `applyReadSilently(ids)` — `markReadMany` plus optimistic patching plus one `refreshCounts`, and no `startOver`. `applyBatchRead` stays untouched for `A` and the header button, which must keep refilling an unread list per `bulk-mark-read`.

### Geometry: scroll by native layout, size the half page by measurement

Scrolling the cursor into view uses `scrollIntoView({ block: "nearest" })` rather than arithmetic over a hardcoded `120px`. Half-page movement measures once (`entriesEl.clientHeight / firstRow.offsetHeight`, floored, minimum 1) with the measurement taken from the DOM rather than a duplicated constant, so the `7.5rem` in the stylesheet stays the only place the row height is defined.

*Alternative considered:* `scrollTop = index * ROW_HEIGHT` with `ROW_HEIGHT = 120`. Rejected — it silently breaks the moment a row height, root font size, or zoom level changes, and it would need the same `scrollIntoView` fallback anyway for the `block: "nearest"` behaviour it does not replicate.

### Suppression is a shared counter, not a DOM query

A tiny reactive module in `src/lib/` exposes `blockKeyboard()` / `unblockKeyboard()` and an `isBlocked` getter. Dialogs, `RowMenu` (while its panel is open), and the help overlay register on mount/open and unregister on close/unmount. `App.vue` checks `isBlocked` first and returns early.

*Alternatives considered:* having `RowMenu` stop keydown propagation (it would have to run in the capture phase to beat the container's listener, and it would still swallow `Escape` handling owned by the container); querying for `.row-menu-panel` in the DOM (couples the resolver to markup). The counter is also what lets `RowMenu` keep its own `Escape` listener untouched.

Text-field focus is checked separately, from the event target: `input`, `textarea`, `select`, or `isContentEditable`. Modifier filtering also lives at the container: a keystroke with `Ctrl`/`Alt`/`Meta` is dropped unless it is `Ctrl-d`/`Ctrl-u`, which are `preventDefault`ed so the webview does not scroll the page.

### Paging intent stays in `App.vue`

`EntryList.vue` keeps rendering and the `IntersectionObserver`. When the container moves the cursor and the landing index is the last loaded row with `hasMore`, `App.vue` calls `loadMore()` and records `pendingAdvance = 1`. When `entries` grows, the pending advance is applied once, landing the cursor on the first row of the new batch; if the batch came back empty, `hasMore` is already false and the advance is dropped. This avoids teaching the presentational component about cursors while keeping one owner of list state.

`G` is bounded by the same rule, for a reason the keyset pagination forces: there is no cursor into the middle of the list, so the only way to reach a distant last row is to append every batch between here and there. A first `G` therefore lands on the last *loaded* row, and each further press at that row appends exactly one batch and moves the cursor to its end. One keypress stays one round trip, and the reader sees the list grow rather than watching a cursor sit still through hundreds of requests.

That leaves the scroll sentinel in the same breath, because a cursor landing on the last loaded row reveals the row *and* the sentinel directly below it. The sentinel's re-ask watcher in `EntryList.vue` therefore had to move to `flush: "post"`: measured before the appended rows are laid out, the sentinel still looks on screen, and one trigger turns into two batches — a jump grew the list by 100 rows instead of 50.

### `J`/`K` and the pane's reset

`ReadingPane.vue` exposes a scroll method (or listens for its own keys when focused) driven by the container, and watches the entry id to reset `scrollTop` to 0 when a different entry is opened. Scrolling must be applied after the new content is rendered, so the reset is a post-flush DOM write, not a data change.

## Risks / Trade-offs

- **Debounced writes can be lost** (application closes, process killed mid-burst) → flush on `window.blur`, on `beforeunload`, and before any list replacement, so the exposed window is a single keystroke gap, and the read state is cosmetic rather than data-critical.
- **`Ctrl-d`/`Ctrl-u` may be intercepted before the webview sees them** on some platforms → if `preventDefault` proves unreliable, fall back to `d`/`u` (single keys are free here) without changing the spec's intent beyond the key names.
- **`?` is already a text character** for some layouts and would be unreachable if the key map later adds an insert-like mode → no such mode is planned; the overlay is also reachable from the sidebar if a discoverability gap appears.
- **The cursor sometimes cannot stand in** (the reader removed the open entry's feed) → that path clears the cursor rather than choosing a neighbour for the reader; the keyboard simply starts from nothing there, exactly like a fresh list.
- **The cursor is not DOM focus**, so a screen reader gets no cursor announcement → accepted for this change; a follow-up can add `aria-activedescendant` on the list without touching the resolver, since the resolver is unaware of rendering.
- **Burst coalescing makes "did my `s`/`m` register?" ambiguous** for actions that are not part of the burst → only cursor movement participates in the burst; `m`, `s`, and `A` act immediately, which keeps single-action feedback instant.
- **Key repeat rates differ per OS and per keyboard** → the flush delay is a tuned constant in one place, and nothing in the spec depends on its exact value.
- **Row-height measurement at first move** can read a transient layout (list virtualized? no — but a scrollbar may appear) → measurement is taken lazily on each half-page move, not cached, so it self-corrects.

## Migration Plan

No schema, storage, or IPC change, and no stored-data migration. The keyboard layer is additive: the mouse paths stay as they are, so removing the listener and the resolver module reverts the change completely. Ship in the order tasks.md sets out (resolver and gating first, motions, then actions, then burst coalescing and the overlay) so each step leaves a working application.

## Open Questions

- Whether `n`/`p` should jump to the next/previous *unread* row (distinct from moving one row and marking it read) is a workflow question that the current spec deliberately leaves out; it can be added later as two more rows in the binding table.
- Whether the count prefix should apply to `m` and `s` (`3m` to mark three rows) is deferred — it is a binding-table change, not an architectural one.
