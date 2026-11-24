<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import type { EntryListItem } from "../api";
import { formatRelativeTime, fullTimestamp } from "../lib/time";

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
}>();

const emit = defineEmits<{
  select: [entry: EntryListItem];
  toggleStar: [entry: EntryListItem];
  refreshAll: [];
  setView: [view: "all" | "unread"];
  loadMore: [];
}>();

/** The scrolling list and the row at its end that asks for another batch. */
const listEl = ref<HTMLElement | null>(null);
const sentinel = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;
let clock: ReturnType<typeof setInterval> | null = null;

/**
 * Clock the relative labels read from. One minute is the finest bucket the
 * labels distinguish, so that is how often the list re-renders them; without
 * this a list left open would keep claiming an entry is "5m" old.
 */
const now = ref(Date.now());

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
watch(
  () => props.loadingMore,
  (loading) => {
    if (!loading && props.hasMore && sentinelVisible()) emit("loadMore");
  },
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
</script>

<template>
  <section class="entry-list">
    <header class="list-header">
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
      <button
        class="refresh-all"
        :disabled="busy"
        title="Refresh all feeds"
        @click="emit('refreshAll')"
      >
        {{ busy ? "Refreshing…" : "Refresh all" }}
      </button>
    </header>

    <ul ref="listEl" class="entries">
      <li
        v-for="entry in entries"
        :key="entry.id"
        class="entry"
        :class="{ active: entry.id === selectedEntryId, unread: !entry.is_read }"
        @click="emit('select', entry)"
      >
        <!-- Decorative: the title carries the meaning. Lazy so covers are only
             fetched for rows that actually come into view. -->
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
          </span>
        </div>
        <button
          class="star"
          :class="{ starred: entry.is_starred }"
          :title="entry.is_starred ? 'Unstar' : 'Star'"
          @click.stop="emit('toggleStar', entry)"
        >
          {{ entry.is_starred ? "\u2605" : "\u2606" }}
        </button>
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
.view-tabs {
  display: flex;
  gap: 2px;
}
.view-tabs button {
  border: none;
  background: transparent;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  cursor: pointer;
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
.entries {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
}
.entry {
  display: flex;
  align-items: flex-start;
  gap: 0.6rem;
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
  padding: 0.55rem 0.75rem;
  border-bottom: 1px solid #eee;
  cursor: pointer;
}
.entry:hover {
  background: #f5f7fa;
}
.entry.active {
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
.star:hover {
  background: #eee;
  color: #d4a017;
}
.star.starred {
  color: #d4a017;
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
