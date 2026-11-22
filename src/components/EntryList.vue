<script setup lang="ts">
import type { Entry } from "../api";

defineProps<{
  entries: Entry[];
  selectedEntryId: number | null;
  view: "all" | "unread";
  busy: boolean;
}>();

const emit = defineEmits<{
  select: [entry: Entry];
  toggleStar: [entry: Entry];
  refreshAll: [];
  setView: [view: "all" | "unread"];
}>();

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

    <ul class="entries">
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
    </ul>
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
</style>
