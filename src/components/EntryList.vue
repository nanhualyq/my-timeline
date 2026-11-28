<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import type { EntryListItem } from "../api";
import { formatRelativeTime, fullTimestamp } from "../lib/time";
import { useHorizontalDrag, usePullToRefresh } from "../lib/gestures";

const props = defineProps<{
  entries: EntryListItem[];
  selectedEntryId: number | null;
  view: "all" | "unread";
  busy: boolean;
  /** False while the Starred view is selected: that view has no unread filter. */
  unreadFilterAvailable: boolean;
  /** Rows loaded so far, for the progress line. */
  viewedCount: number;
  /** Entries matching the current view and scope — not a global total. */
  total: number;
  /** Another batch exists beyond the loaded rows. */
  hasMore: boolean;
  /** A further batch is in flight. */
  loadingMore: boolean;
  /** Display titles by feed id (custom title applied server-side). */
  feedTitles: Record<number, string>;
  /** False in a single feed's view, where the label repeats the sidebar. */
  showFeedLabel: boolean;
  /** True when the current list is a triage queue (a feed, a folder, or the
   *  global unread view), where batch marking belongs. */
  batchActionsAvailable: boolean;
  /** True on the one-pane layout: the header hosts the drawer toggle. */
  narrow: boolean;
  /** True where the primary pointer is a finger: rows stop depending on hover
   *  and their actions are gestures and the long-press menu. */
  touch: boolean;
}>();

const emit = defineEmits<{
  select: [entry: EntryListItem];
  toggleStar: [entry: EntryListItem];
  refreshAll: [];
  /** Touch layout only: a pull at the top of the list refreshes its scope. */
  pullRefresh: [];
  setView: [view: "all" | "unread"];
  loadMore: [];
  /** Mark every unread row the list is showing. */
  markRendered: [];
  /** Mark the unread rows above the row at this index, excluding that row. */
  markAbove: [index: number];
  /** Narrow layout only: the reader wants the feed drawer. */
  openNav: [];
  /** Touch layout only: a long press asked for the row's menu. */
  contextMenu: [entry: EntryListItem, index: number];
}>();

/** The scrolling list and the row at its end that asks for another batch. */
const listEl = ref<HTMLElement | null>(null);
const sentinel = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;
let clock: ReturnType<typeof setInterval> | null = null;

/**
 * Keep the cursor row inside the scroll port. The list does not decide where
 * the cursor is; it only makes the row the container chose visible.
 */
function revealCursor(id: number | null) {
  if (id === null) return;
  const row = listEl.value?.querySelector<HTMLElement>(
    `.entry[data-entry-id="${id}"]`,
  );
  // jsdom has no scrollIntoView; the guard keeps the render path safe there.
  row?.scrollIntoView?.({ block: "nearest" });
}

// After the render, so the row exists when a jump appends new rows.
watch(() => props.selectedEntryId, revealCursor, { flush: "post" });

/**
 * Reveal a row on request, not only when the cursor moves.
 *
 * A move made while the list was hidden — a reader swiping through articles on
 * the narrow layout — cannot scroll it: `scrollIntoView` does nothing on a
 * `display: none` subtree. The container therefore asks once the list is back
 * on screen, which is also the only moment the reveal is worth anything.
 */
defineExpose({ visibleRowCount, revealEntry: revealCursor });

/**
 * Rows that fit in the list box. Measured from a rendered row rather than from
 * the row height in the stylesheet, so the stylesheet stays the only place
 * that height is defined.
 */
function visibleRowCount(): number {
  const box = listEl.value;
  const row = box?.querySelector<HTMLElement>(".entry");
  if (!box || !row) return 0;
  const rowHeight = row.offsetHeight;
  if (rowHeight <= 0) return 0;
  return Math.max(1, Math.floor(box.clientHeight / rowHeight));
}



/**
 * Clock the relative labels read from. One minute is the finest bucket the
 * labels distinguish, so that is how often the list re-renders them; without
 * this a list left open would keep claiming an entry is "5m" old.
 */
const now = ref(Date.now());

/**
 * The row gestures. One instance for the whole list rather than one per row:
 * the samples arrive on the window, and only the surface under the finger is
 * ever reported.
 *
 * A leftward drag toggles the row's star, a rightward drag marks the rendered
 * rows above it read — the same two actions the pointer layout puts on the
 * row's controls, so the two layouts cannot drift apart.
 */
const {
  activeKey: dragKey,
  offset: dragOffset,
  dragging: dragActive,
  onPointerDown: startDrag,
} = useHorizontalDrag({
  enabled: () => props.touch,
  onCommit: (target, direction) => {
    const index = props.entries.findIndex((row) => row.id === target.key);
    if (index < 0) return;
    const row = props.entries[index];
    if (direction === "left") emit("toggleStar", row);
    else emit("markAbove", index);
  },
  onLongPress: (target) => {
    const index = props.entries.findIndex((row) => row.id === target.key);
    if (index < 0) return;
    emit("contextMenu", props.entries[index], index);
  },
});

/** Where the dragged surface sits, in pixels. Only the row under the finger
 *  moves; every other row keeps the resting position. */
function dragStyle(entry: EntryListItem): Record<string, string> {
  const dx = dragKey.value === entry.id ? dragOffset.value : 0;
  return { "--drag-x": `${dx}px` };
}

/** Which action the drag in progress would commit, for the revealed panel. */
function dragIntentFor(entry: EntryListItem): "left" | "right" | null {
  if (dragKey.value !== entry.id || !dragActive.value) return null;
  return dragOffset.value < 0 ? "left" : "right";
}

function onRowPointerDown(
  event: PointerEvent,
  entry: EntryListItem,
  _index: number,
) {
  if (!props.touch) return;
  startDrag(event, { key: entry.id });
}

/**
 * The pull at the top of the list. It replaces the refresh button on the
 * one-pane layout, so it is enabled exactly where that button is absent.
 */
const {
  offset: pullOffset,
  pulling: pullActive,
  onTouchStart: onListTouchStart,
  onTouchMove: onListTouchMove,
  onTouchEnd: onListTouchEnd,
  onTouchCancel: onListTouchCancel,
} = usePullToRefresh({
  enabled: () => props.touch && props.narrow,
  port: () => listEl.value,
  onRefresh: () => emit("pullRefresh"),
});

/** Displacement of the list while it is being pulled. */
function pullStyle(): Record<string, string> | undefined {
  if (!pullActive.value && pullOffset.value === 0) return undefined;
  return { transform: `translateY(${pullOffset.value}px)` };
}

function sentinelVisible(): boolean {
  const row = sentinel.value?.getBoundingClientRect();
  const box = listEl.value?.getBoundingClientRect();
  if (!row || !box) return false;
  return row.top <= box.bottom && row.bottom >= box.top;
}

onMounted(() => {
  clock = setInterval(() => {
    now.value = Date.now();
  }, 60_000);
  if (!listEl.value) return;
  observer = new IntersectionObserver(
    (records) => {
      if (records.some((r) => r.isIntersecting) && props.hasMore) {
        emit("loadMore");
      }
    },
    // Start loading slightly before the reader reaches the end.
    { root: listEl.value, rootMargin: "150px 0px" },
  );
  if (sentinel.value) observer.observe(sentinel.value);
});

// An observer only reports crossings, so after a batch lands it would stay
// silent while the sentinel is still on screen. Ask again instead of
// stalling — the list keeps growing until the sentinel scrolls away.
//
// Post-flush, deliberately: the measurement has to see the rows that just
// landed. Measured before the re-render, the sentinel still looks on screen
// because the appended rows are not in the DOM yet, and one trigger becomes two
// batches — a single `G` would grow the list by 100 rows instead of 50.
watch(
  () => props.loadingMore,
  (loading) => {
    if (!loading && props.hasMore && sentinelVisible()) emit("loadMore");
  },
  { flush: "post" },
);

onBeforeUnmount(() => {
  observer?.disconnect();
  if (clock !== null) clearInterval(clock);
});

/** Covers that failed to load, by entry id. Hiding the image keeps the row
 *  structure (and therefore the row height) identical to a coverless entry. */
const brokenCovers = reactive(new Set<number>());

function markCoverBroken(entryId: number) {
  brokenCovers.add(entryId);
}

function coverFor(entry: EntryListItem): string | null {
  if (!entry.image_url || brokenCovers.has(entry.id)) return null;
  return entry.image_url;
}

function feedLabel(entry: EntryListItem): string {
  return props.showFeedLabel ? (props.feedTitles[entry.feed_id] ?? "") : "";
}

/**
 * The row's time. `sort_key` is the fallback rather than nothing: it is what the
 * backend ordered the row by (`published_at` coalesced with arrival time), so an
 * entry without a publication date still shows when it arrived.
 */
function rowTime(entry: EntryListItem): string {
  return entry.published_at ?? entry.sort_key;
}

function fmtTime(entry: EntryListItem): string {
  return formatRelativeTime(rowTime(entry), now.value);
}

function fullTime(entry: EntryListItem): string {
  return fullTimestamp(rowTime(entry));
}

/** Entries the header action would mark: everything unread on this page. */
const renderedUnread = computed(
  () => props.entries.filter((e) => !e.is_read).length,
);

/** Unread entries above the row at `index`, which is exactly what the row
 *  action reaches: the list only ever appends downwards, so every newer entry
 *  of the same list is already loaded. */
function unreadAbove(index: number): number {
  return props.entries.slice(0, index).filter((e) => !e.is_read).length;
}

/** What a rightward drag on this row would do, in the reader's words. */
function markAboveLabel(index: number): string {
  const count = unreadAbove(index);
  return count === 0 ? "Nothing unread above" : `Mark ${plural(count, "entry", "entries")} above read`;
}

/** A touch row has no star button, so its starred state is carried by a mark.
 *  Where the button exists it already says the same thing — showing both would
 *  state it twice. */
function showsStarMark(entry: EntryListItem): boolean {
  return props.touch && entry.is_starred;
}

/** `1 entry` / `2 entries`: these labels are read out by screen readers and
 *  shown as tooltips, so "Mark 1 entries" is a visible mistake. */
function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}
</script>

<template>
  <section class="entry-list">
    <header class="list-header" :class="{ 'narrow-header': narrow }">
      <button
        v-if="narrow"
        type="button"
        class="nav-toggle touch-target"
        aria-label="Open folders and feeds"
        @click="emit('openNav')"
      >
        <span aria-hidden="true">&#9776;</span>
      </button>
      <div class="view-tabs">
        <button
          :class="{ active: view === 'all' }"
          @click="emit('setView', 'all')"
        >
          All
        </button>
        <button
          v-if="unreadFilterAvailable"
          :class="{ active: view === 'unread' }"
          @click="emit('setView', 'unread')"
        >
          Unread
        </button>
      </div>
      <div class="list-actions">
        <!-- The one-pane layout refreshes by pulling the list instead, so the
             control is not offered there. -->
        <button
          v-if="!narrow"
          class="refresh-all"
          :disabled="busy"
          title="Refresh all feeds"
          aria-label="Refresh all feeds"
          @click="emit('refreshAll')"
        >
          {{ busy ? "Refreshing…" : "Refresh all" }}
        </button>
        <button
          v-if="batchActionsAvailable"
          class="mark-rendered"
          :disabled="busy || renderedUnread === 0"
          :title="`Mark ${plural(renderedUnread, 'unread entry', 'unread entries')} on this page as read`"
          :aria-label="`Mark ${plural(renderedUnread, 'unread entry', 'unread entries')} on this page as read`"
          @click="emit('markRendered')"
        >
          Mark {{ renderedUnread }} read
        </button>
      </div>
    </header>

    <ul
      ref="listEl"
      class="entries"
      :style="pullStyle()"
      @touchstart="onListTouchStart"
      @touchmove="onListTouchMove"
      @touchend="onListTouchEnd"
      @touchcancel="onListTouchCancel"
    >
      <li
        v-for="(entry, index) in entries"
        :key="entry.id"
        :data-entry-id="entry.id"
        class="entry"
        :class="{
          active: entry.id === selectedEntryId,
          unread: !entry.is_read,
          'narrow-row': narrow,
          'touch-row': touch,
          dragging: dragIntentFor(entry) !== null,
        }"
        :style="dragStyle(entry)"
        @click="emit('select', entry)"
        @pointerdown="onRowPointerDown($event, entry, index)"
      >
        <!-- Behind the row and revealed by the drag: what a release would do.
             On a touch layout these replace the row's two controls, so the row
             gives no width to either until a finger asks for one. -->
        <template v-if="touch">
          <div
            class="swipe-panel leading"
            :class="{ revealed: dragIntentFor(entry) === 'right' }"
            aria-hidden="true"
          >
            {{ markAboveLabel(index) }}
          </div>
          <div
            class="swipe-panel trailing"
            :class="{ revealed: dragIntentFor(entry) === 'left' }"
            aria-hidden="true"
          >
            {{ entry.is_starred ? "Unstar" : "Star" }}
          </div>
        </template>
        <div class="entry-surface">
          <!-- Decorative: the title carries the meaning. Lazy so covers are
               only fetched for rows that actually come into view. -->
          <img
            v-if="coverFor(entry)"
            class="entry-cover"
            :src="coverFor(entry) ?? ''"
            alt=""
            loading="lazy"
            @error="markCoverBroken(entry.id)"
          />
          <div class="entry-body">
            <span class="entry-title">{{ entry.title }}</span>
            <span v-if="entry.excerpt" class="entry-excerpt">{{ entry.excerpt }}</span>
            <span class="entry-meta">
              <span v-if="feedLabel(entry)" class="entry-feed">{{ feedLabel(entry) }}</span>
              <span class="entry-date" :title="fullTime(entry)">{{ fmtTime(entry) }}</span>
              <span v-if="showsStarMark(entry)" class="entry-star-mark">
                <span aria-hidden="true">&#9733;</span>
                <span class="sr-only">Starred</span>
              </span>
            </span>
          </div>
          <!-- The pointer layout's controls: a pointer aims at a compact
               button and hovers to reveal one. A touch row has neither. -->
          <template v-if="!touch">
            <button
              class="star"
              :class="{ starred: entry.is_starred }"
              :title="entry.is_starred ? 'Unstar' : 'Star'"
              :aria-label="entry.is_starred ? 'Unstar entry' : 'Star entry'"
              @click.stop="emit('toggleStar', entry)"
            >
              {{ entry.is_starred ? "★" : "☆" }}
            </button>
            <!-- Floats over the row and reserves no column, so a row without
                 the action keeps its previous layout. The arrow carries the
                 count the action would mark. -->
            <button
              v-if="batchActionsAvailable && unreadAbove(index) > 0"
              class="mark-above"
              :disabled="busy"
              :title="`Mark the ${plural(unreadAbove(index), 'unread entry', 'unread entries')} above this one as read`"
              :aria-label="`Mark ${plural(unreadAbove(index), 'entry', 'entries')} above as read`"
              @click.stop="emit('markAbove', index)"
            >
              &#8593;{{ unreadAbove(index) }}
            </button>
          </template>
        </div>
      </li>
      <li v-if="entries.length === 0" class="empty">No entries.</li>
      <!-- Always mounted so the observer wiring never has to be rebuilt;
           hidden only while there is nothing to scroll through. -->
      <li
        ref="sentinel"
        class="sentinel"
        :class="{ hidden: entries.length === 0 }"
        aria-live="polite"
      >
        <span v-if="loadingMore">Loading…</span>
        <span v-else-if="hasMore">Scroll for more</span>
        <span v-else>End of list</span>
      </li>
    </ul>

    <footer class="list-footer">
      <span class="progress">{{ viewedCount }} / {{ total }} entries viewed</span>
    </footer>
  </section>
</template>

<style scoped>
.entry-list {
  display: flex;
  flex-direction: column;
  border-right: 1px solid #ddd;
  min-width: 0;
  /* Grid items default to min-height: auto, which keeps this column as tall
     as its content; opt out so the list below can scroll on its own. */
  min-height: 0;
  overflow: hidden;
}
.list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid #ddd;
}
/* Narrow: the header carries an extra control and has less width, so it wraps
   instead of clipping the actions out of reach. */
.narrow-header {
  flex-wrap: wrap;
  row-gap: 0.4rem;
}
.nav-toggle {
  border: 1px solid #ccc;
  background: #f5f5f5;
  border-radius: 4px;
  cursor: pointer;
  font-size: 1rem;
  line-height: 1;
  padding: 0.45rem 0.6rem;
}
.view-tabs {
  display: flex;
  gap: 2px;
}
.list-actions {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}
.view-tabs button {
  border: none;
  background: transparent;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  cursor: pointer;
}
/* A touch layout has no pointer to hover with, and a tap leaves the
   last-touched row stuck in its hover state on Android. Every hover rule in
   this component is therefore gated, and :active carries the feedback. */
@media (hover: hover) and (pointer: fine) {
  .view-tabs button:hover {
    background: #eef2f7;
  }
  .refresh-all:hover:not(:disabled) {
    background: #eaeaea;
  }
  .mark-rendered:hover:not(:disabled) {
    background: #e2edff;
  }
}
.view-tabs button.active {
  background: #2d7ff9;
  color: #fff;
}
.refresh-all {
  border: 1px solid #ccc;
  background: #f5f5f5;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  cursor: pointer;
}
.refresh-all:disabled {
  opacity: 0.6;
  cursor: default;
}
.mark-rendered {
  border: 1px solid #2d7ff9;
  background: #f2f7ff;
  color: #1d5fc4;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  cursor: pointer;
}
.mark-rendered:disabled {
  opacity: 0.45;
  cursor: default;
}
.entries {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
}
.entry {
  /* The surface fills the row and translates while it is dragged; the panels
     behind it are what the drag reveals. */
  position: relative;
  display: flex;
  align-items: stretch;
  /* Fixed height is what keeps lazy image loading from shifting the rows the
     reader is looking at: a cover arriving late cannot resize its row.
     The number is the worst case, not an arbitrary preference: 2-line title
     (2 x 1.35 x 16px = 44) + 2-line excerpt (2 x 1.35 x 13px = 36) + meta line
     (15) + gaps (5) + vertical padding (18) lands at ~118px, so 120 leaves a
     couple of pixels of slack for line-box rounding. Expressed in rem (7.5rem
     at the default 16px root) so the row scales with the root font size the
     same way the title, excerpt, and meta line do. Undersizing it lets the
     meta line paint over the next row; `overflow: hidden` is the backstop for
     larger system fonts. */
  height: 7.5rem;
  overflow: hidden;
  box-sizing: border-box;
  /* The pointer layout's row-level batch action floats over the row on hover
     instead of taking a flex column, so the row layout does not pay for a
     control the reader only uses occasionally. */
  border-bottom: 1px solid #eee;
  cursor: pointer;
  /* Vertical scrolling stays the browser's; the horizontal axis is ours. */
  touch-action: pan-y;
}
/* A long press on a touch row is the row's menu, so the row's text must not
   start a selection instead. */
.entry.touch-row {
  user-select: none;
  -webkit-touch-callout: none;
}
.entry-surface {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
  flex: 1;
  min-width: 0;
  padding: 0.55rem 0.75rem;
  box-sizing: border-box;
  /* Opaque, so the panels behind it are only visible where a drag has moved
     it aside. */
  background: #fff;
  transform: translateX(var(--drag-x, 0px));
}
.entry-surface,
.swipe-panel {
  /* A drag must not fight the browser's own scroll animation. */
  transition: transform 120ms ease-out;
}
.entry.dragging .entry-surface,
.entry.dragging .swipe-panel {
  transition: none;
}
.entry.dragging .entry-surface {
  /* The finger owns the surface: no text selection under it either. */
  user-select: none;
}
/* What the drag would commit. Behind the surface, in the direction the finger
   is pulling the row away from. */
.swipe-panel {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  padding: 0 1rem;
  font-size: 0.8rem;
  font-weight: 600;
  opacity: 0;
  pointer-events: none;
}
.swipe-panel.revealed {
  opacity: 1;
}
.swipe-panel.leading {
  justify-content: flex-start;
  background: #e8f1ff;
  color: #1d5fc4;
}
.swipe-panel.trailing {
  justify-content: flex-end;
  background: #fff4d8;
  color: #8a6508;
}
@media (hover: hover) and (pointer: fine) {
  .entry:hover .entry-surface {
    background: #f5f7fa;
  }
}
.entry:active .entry-surface {
  background: #eef2f7;
}
.entry.narrow-row {
  /* The row is a tap target, so it must be at least as tall as one even when
     its content is short; the existing fixed height already exceeds 44px. */
  min-height: 44px;
}
.entry.active .entry-surface {
  background: #e3efff;
}
.entry-cover {
  /* Stretched rather than sized: the row height is fixed, so the cover simply
     fills the row's interior and the two can never drift apart. */
  flex: 0 0 5.25rem;
  width: 5.25rem;
  align-self: stretch;
  height: auto;
  object-fit: cover;
  border-radius: 4px;
  background: #f0f0f0;
}
.entry-body {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.entry-title {
  font-weight: 600;
}
/* Two lines max for both, so a long title or excerpt cannot stretch the row.
   max-height is the fallback for engines without -webkit-line-clamp. */
.entry-title,
.entry-excerpt {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  max-height: 2.7em;
  line-height: 1.35;
}
.entry-excerpt {
  color: #555;
  font-size: 0.82rem;
  /* May shrink when a taller title or a larger system font eats the budget:
     losing the tail of the excerpt is better than losing the feed and time. */
  flex: 0 1 auto;
  min-height: 0;
}
.entry.unread .entry-title::before {
  content: "● ";
  color: #2d7ff9;
  font-size: 0.7em;
  vertical-align: middle;
}
.entry-meta {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  margin-top: auto;
  font-size: 0.75rem;
  line-height: 1.2;
  color: #888;
  min-width: 0;
  flex-shrink: 0;
}
.entry-feed {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.entry-feed + .entry-date::before {
  content: "· ";
}
.entry-date {
  flex-shrink: 0;
}
/* A touch row's starred state, once the row no longer carries a star button.
   The glyph is decorative; the label beside it is what a screen reader reads. */
.entry-star-mark {
  flex-shrink: 0;
  color: #d4a017;
}
.entry-date + .entry-star-mark::before,
.entry-feed + .entry-star-mark::before {
  content: "· ";
  color: #888;
}
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
.star {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 0.95rem;
  line-height: 1;
  padding: 0.1rem 0.2rem;
  border-radius: 4px;
  color: #999;
  flex-shrink: 0;
}
@media (hover: hover) and (pointer: fine) {
  .star:hover {
    background: #eee;
    color: #d4a017;
  }
}
.star.starred {
  color: #d4a017;
}
.mark-above {
  /* Floats over the row's top-right corner, just left of the star: no column
     is reserved, so a row without the action looks exactly like before. */
  position: absolute;
  top: 0.4rem;
  right: 1.75rem;
  cursor: pointer;
  font-size: 0.7rem;
  line-height: 1;
  padding: 0.22rem 0.35rem;
  border: 1px solid #bcd4f7;
  border-radius: 10px;
  background: #eef4ff;
  color: #1d5fc4;
  /* Hidden until the reader points at the row. */
  opacity: 0;
  pointer-events: none;
}
/* Only a device with a real pointer can hover, so only there the control is
   revealed by pointing at the row. A touch row does not render it at all. */
@media (hover: hover) and (pointer: fine) {
  .entry:hover .mark-above,
  .mark-above:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }
  .mark-above:hover:not(:disabled) {
    background: #dbe9ff;
  }
}
.mark-above:disabled {
  opacity: 0;
  pointer-events: none;
  cursor: default;
}
.empty {
  padding: 1rem 0.75rem;
  color: #888;
  font-style: italic;
}
.sentinel {
  padding: 0.6rem 0.75rem;
  text-align: center;
  color: #888;
  font-size: 0.78rem;
  font-style: italic;
}
.sentinel.hidden {
  display: none;
}
.list-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.4rem 0.75rem;
  border-top: 1px solid #ddd;
  font-size: 0.78rem;
  color: #666;
  flex-shrink: 0;
}
</style>
