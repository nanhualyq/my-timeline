<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { EntryListItem } from "../api";

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

function sentinelVisible(): boolean {
  const row = sentinel.value?.getBoundingClientRect();
  const box = listEl.value?.getBoundingClientRect();
  if (!row || !box) return false;
  return row.top <= box.bottom && row.bottom >= box.top;
}

onMounted(() => {
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

onBeforeUnmount(() => observer?.disconnect());

function fmtDate(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? "" : d.toLocaleDateString();
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
        <span class="entry-title">{{ entry.title }}</span>
        <span class="entry-date">{{ fmtDate(entry.published_at) }}</span>
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
  justify-content: space-between;
  gap: 0.5rem;
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
.entry-title {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.entry.unread .entry-title::before {
  content: "● ";
  color: #2d7ff9;
  font-size: 0.7em;
  vertical-align: middle;
}
.entry-date {
  color: #888;
  font-size: 0.8rem;
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
