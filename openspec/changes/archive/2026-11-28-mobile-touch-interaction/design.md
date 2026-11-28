## Context

See `proposal.md` — Why. Current state that shapes the approach:

- `src/lib/viewport.ts` derives one `Layout = "narrow" | "wide"` from `NARROW_MAX_WIDTH = 800` and publishes it as `data-layout` on `<html>`; layout CSS keys off that attribute so the number never appears in a media query. A second, styling-only axis already exists as `HOVER_MEDIA_QUERY` (`(hover: hover) and (pointer: fine)`), consumed by `useMediaQuery` in `App.vue` as `hoverInput`.
- Structure and interaction are currently the same decision: `narrow` gates the one-pane `v-show` switching, the drawer, the reader screen, the back-stack layers, `showFeedLabel`-independent row styling, and the 44 px touch targets in `App.vue`'s global CSS. Every hover rule is already gated behind `@media (hover: hover) and (pointer: fine)`.
- `EntryList.vue` renders the row's `☆` star button unconditionally and the `↑N` batch chip on narrow rows as `opacity: 1; pointer-events: auto`, with `.narrow-row .entry-body { padding-right: 2.5rem }` compensating for the overlay.
- `App.vue` already owns everything a swipe needs: `stepCursor(delta)` opens the neighbouring row, marks it read into a coalesced burst, loads the next batch when the cursor walks off the loaded rows, and drops stale content responses. `pendingAdvance` is the existing mechanism for "keep moving after the batch lands".
- `markReadMany` is called from exactly two places — `markRenderedRead()` and `markAboveRead(index)` — and both funnel through `applyBatchRead(ids)`.
- `back-stack.ts` mirrors open layers into `history` so Android's back action closes one layer at a time; `openLayers` in `App.vue` is the single list of layers.
- `RowMenu.vue` already provides a dropdown with document-click dismissal and `blockKeyboard()`; it is anchored to a trigger button and is not a touch-point presentation.

## Goals / Non-Goals

**Goals:**

- Split the single "narrow" decision into a structure decision (one pane vs three columns) and an interaction decision (touch vs pointer), so a wide coarse-pointer device keeps its three columns but gets touch affordances.
- Add horizontal drags to entry rows and to the reading pane without touching the existing cursor, burst, batching, or pagination model.
- Make bulk marking recoverable, since it is now one mis-gesture away from a scroll.
- Keep every removed affordance reachable: by gesture, by long-press menu, or by a visible control.

**Non-Goals:**

- No gesture library or new runtime dependency; no backend, storage, or sync changes.
- No redesign of the wide pointer layout beyond routing its row controls through the same components.
- No general gesture framework — only the horizontal drags and the long press this change needs.
- Not fixing `markAboveRead`'s count semantics or `bulk-mark-read`'s "rendered entries only" rule, both of which stay as they are.

## Decisions

### 1. Two independent axes, with a second width threshold for coarse pointers

`viewport.ts` gains `COARSE_MEDIA_QUERY = "(pointer: coarse)"` and keeps `NARROW_MEDIA_QUERY` and `NARROW_MAX_WIDTH` as they are. It publishes two attributes: `data-layout` (`narrow` | `wide`, unchanged meaning) and `data-input` (`touch` | `pointer`).

| viewport | primary pointer | layout (columns) | input (affordances) |
|---|---|---|---|
| width < 800 | any | narrow — one pane | touch |
| 800 <= width < 1024 | coarse | narrow — one pane | touch |
| width >= 1024 | coarse | wide — three columns | touch |
| width >= 800 | fine | wide — three columns | pointer |

The 1024 threshold exists only for coarse-pointer devices: a landscape phone measures roughly 915 x 412 CSS px and must not receive three columns inside 412 px of height, while a tablet in landscape must keep the three-column layout `mobile-shell` already promises.

*Alternatives considered:* a single `inputMode` deciding both (rejected — it would demote an iPad in landscape to one pane and contradict `mobile-shell`); keeping width as the only key (rejected — leaves the landscape-phone hole, where hover-revealed row actions are unreachable). `(pointer: coarse)` rather than `(hover: none)` because a device that has hover but no fine pointer is not something the app targets, and `pointer: coarse` is the more widely supported of the two.

CSS keeps its existing `@media (hover: hover) and (pointer: fine)` gates for hover styling, and the target-size rules in `App.vue` that currently key off `:root[data-layout="narrow"]` move to `:root[data-input="touch"]`, so a wide tablet's rows are tappable too.

### 2. Gestures live in one module as a pure state machine plus a thin DOM adapter

`src/lib/gestures.ts` exports:

- `DRAG_SLOP = 8` — movement below this is a tap.
- `DIRECTION_LOCK = 10` with `|dx| > |dy| * 1.5` — the drag must be clearly horizontal before the row claims it.
- `COMMIT_RATIO = 0.4` and `COMMIT_MIN = 72` — commit once `|dx| >= min(0.4 * rowWidth, ...)`; the row width is measured once at drag start.
- `EDGE_GUTTER = 32` — a drag that begins within this many pixels of either screen edge is not claimed, leaving Android's back gesture intact.
- `resolveDrag(state, sample) -> "pending" | "scrolling" | "dragging" | "commit-left" | "commit-right" | "cancel"` — no DOM, no timers, mirroring how `keymap.ts` separates the grammar from the dispatcher.
- `useHorizontalDrag(el, { onCommit, onProgress })` — a Vue composable that feeds pointer samples to the state machine using passive listeners.

Pointer Events rather than Touch Events: one code path for finger and stylus, already supported by the Android WebView this ships to. Rows and `.content` get `touch-action: pan-y`, so the browser keeps native vertical scrolling and the composable never needs `preventDefault` or a non-passive listener — there is no scroll-jank risk and no pointer-capture dance.

*Alternative considered:* a dependency such as a generic gesture library. Rejected — the vocabulary is four behaviours, and a pure state machine is what the existing test suite can drive without a browser.

### 3. Direction split, not an action rail

The row is not restructured into columns. Each `<li>` gains two sibling panels behind its content (leading and trailing), and the content is translated by a CSS custom property while dragging:

```
<li class="entry">
  <div class="swipe-panel leading">↑3 · Mark 3 above read</div>
  <div class="swipe-panel trailing">☆ · Star</div>
  <div class="entry-surface"> cover · body · (pointer only: star, ↑N) </div>
</li>
```

`transform: translateX(var(--drag-x))` keeps the row out of the layout path, so dragging cannot shift neighbours or interact with lazy cover loading and the fixed row height. Only the panel matching the current direction is visible, which is what makes the in-progress feedback rule (`touch-gestures`) testable. On commit the row animates back and the action is applied; a committed gesture sets a one-tick `suppressClick` flag so the release does not also open the entry.

On the pointer layout the same component keeps its existing `☆` button and hover-revealed `↑N` chip — the gesture module is not wired up there at all, and the row keeps its reserved padding only in that mode.

### 4. Pane swipes delegate to `stepCursor`

`ReadingPane.vue` emits `previous` / `next`; `App.vue` handles them as `void stepCursor(-1)` / `void stepCursor(1)`. This is the whole implementation of "swipe switches entries": batching at the end of the list (`pendingAdvance` + `loadMore`), read coalescing (`pendingReadIds`), the stale-response guard, and marking-on-open all come for free and stay in one place. The pane's own horizontal drage uses the same `gestures.ts` state machine, applied to its content region, with `EDGE_GUTTER` doubled because the reading screen has no row-level vertical scroll to compete with but does sit against the system edge.

The pane exposes nothing new; `scrollByScreens` stays for the keyboard path.

### 5. Star control in the reading pane on every layout

`ReadingPane.vue` gains an icon button (`☆` / `★` with `aria-label` of `Star entry` / `Unstar entry`) next to the existing actions, on every layout. Layout-conditional rendering was rejected: it would put a second behavioural branch into the one component both layouts share, and the pane is the natural place to star while reading on desktop too. Nothing about the desktop row's star control changes, so no duplicate-state confusion arises.

### 6. Starred state becomes a marker, only where the button is gone

When the row has no star button (touch layout), an already-starred row shows `★` inside `.entry-meta`. The marker is presentational (`aria-hidden` on the glyph with the state carried by text in the row's accessible name, or a visually hidden `Starred` label) and is not interactive. Where the star button exists (pointer layout) the marker is not rendered — the button's own appearance carries the state — so the two never double up, which is what the `entry-row-presentation` delta states.

### 7. Long-press menu as a touch-point sheet reusing `RowMenu`'s contract

A long press (`LONG_PRESS_MS = 500`) opens a menu whose items have `RowMenu.vue`'s `{ key, label, danger }` shape, so the item model and the `select` event are shared. The presentation is a bottom sheet rather than a trigger-anchored dropdown, because a touch has no trigger. It registers in `openLayers` so the system back action closes it before anything under it, and it calls `blockKeyboard()` so the container stops moving the cursor behind it.

This menu is the accessibility path: TalkBack and similar services consume horizontal drags, so without it the two row actions would be unreachable for those readers.

*Alternative considered:* teaching the swipe with a first-run coach mark only. Rejected as the sole mitigation — it solves discoverability for sighted direct-touch users and nothing for assistive technology.

### 8. Single-level undo owned by `App.vue`

`App.vue` holds `pendingUndo: { ids: number[], label: string, timer }` — at most one. `applyBatchRead(ids)` records the ids it is about to mark (they are already computed there) and arms a `UNDO_WINDOW_MS = 3000` timer; `markRenderedRead()` and `markAboveRead()` therefore both get undo without changing their signatures, and the gesture path inherits it for free because it calls `markAboveRead`.

Undo writes `setRead(id, false)` for each recorded id, patches the rows via `patchEntry`, then refreshes counts. A second batch replaces the pending undo rather than stacking, which keeps the state a single slot and matches what a 3-second window can realistically contain.

`setRead` per id rather than `applyReadSilently` — the silent path deliberately skips count and list refresh, which is the opposite of what undo needs.

### 9. Returning from the reader reveals the open entry

`EntryList.vue` exposes `revealEntry(id)` wrapping the existing `revealCursor` logic. `App.vue` calls it post-flush when the reader screen closes, because the current `watch(() => props.selectedEntryId, revealCursor)` runs and is a no-op while the list is `v-show`-hidden (`scrollIntoView` on a hidden element does nothing). Same guard for jsdom, which has no `scrollIntoView`.

## Risks / Trade-offs

- **A committed drag also fires a click** → `suppressClick` cleared on the next tick, and `:active` styling gated behind a `dragging` class so the row does not flash as pressed.
- **Undo in an unread-filtered list cannot restore rows that were dropped** — a batch mark reloads the list, so the marked rows are gone; restoring their read state alone would leave the reader staring at a list that does not show them. → Undo triggers a reload of the current list (accepting a scroll-position jump) when the list is filtered to unread, and patches in place otherwise.
- **Two overlapping batches** could make an undo ambiguous → single-slot undo; the newer batch replaces the older pending undo, and the withdrawn control stops claiming to restore the older one.
- **Gesture and system back collide on the left edge** → `EDGE_GUTTER` is not claimed at all; the rightward (batch mark) direction is the one that would have collided most, and it is the crossing that also requires `DIRECTION_LOCK` to pass first.
- **TalkBack consumes drags** → the long-press menu is the documented path, and it is the one the specs make mandatory.
- **A WebView without Pointer Events** → the composable attaches nothing, rows fall back to tap-only, and the long-press menu still covers both actions; no error surface, no half-mounted listener.
- **`touch-action: pan-y` on `.content`** disables native horizontal panning for wide tables or code blocks inside articles → accepted; the reading pane is a text surface and the horizontal axis is now navigation. If it bites, the article can opt back into `pan-x` for `pre > code` later without touching the gesture module.
- **Long press vs text selection inside article** → the long press is only wired on entry rows, never on the article, so selection is untouched (`touch-gestures` has a scenario for it).
- **Row width measured at drag start** → a rotation or viewport change mid-drag would use a stale width; the commit threshold is re-clamped at release, which is enough for a gesture that rarely outlives a second.

## Migration Plan

Single-slice change to the frontend; no data or protocol migration. Landing order keeps the app usable at every step:

1. `viewport.ts` publishes `data-input` and the coarse threshold; `App.vue`'s target-size selectors move to it. No behaviour change yet on any existing device.
2. `gestures.ts` + composable, tested in isolation.
3. Undo bar, covering the existing header and hover-only paths before any gesture exists — this way bulk marking becomes recoverable before it becomes easier to trigger by accident.
4. Row panels and gesture wiring; removal of the touch-layout `☆` and `↑N`, with the starred marker and long-press menu landing in the same step so no state or action is lost even momentarily.
5. Reading pane: star control, then swipe wiring to `stepCursor`.
6. Return-to-list reveal.

Rollback is per step: steps 1-3 are additive and can ship alone; step 4 is the only one that removes a control, and it is guarded by the long-press menu landing with it.

## Open Questions

- The exact form of the reading screen's indication that neighbouring entries exist (`touch-gestures` requires an indication, not a specific control). Leaving it to implementation: a disabled-when-unavailable previous/next pair in the pane header is the cheap option, but whether it earns its space on a phone is worth judging against the rendered screen.
- Whether `UNDO_WINDOW_MS = 3000` feels right on a device; it is a single constant and does not change any requirement.
