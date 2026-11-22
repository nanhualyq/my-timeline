<script setup lang="ts">
import type { Entry, Feed } from "../api";

defineProps<{
  entry: Entry | null;
  feed: Feed | null;
}>();

const emit = defineEmits<{
  toggleRead: [];
  openLink: [url: string];
}>();
</script>

<template>
  <section class="reading-pane">
    <template v-if="entry">
      <header class="pane-header">
        <div class="pane-meta">
          <h2 class="pane-title">{{ entry.title }}</h2>
          <p class="pane-sub">
            <span v-if="feed">{{ feed.title }}</span>
            <span v-if="entry.published_at">
              · {{ new Date(entry.published_at).toLocaleString() }}
            </span>
          </p>
        </div>
        <div class="pane-actions">
          <a
            v-if="entry.link"
            class="btn"
            :href="entry.link"
            target="_blank"
            rel="noopener noreferrer"
            @click.prevent="emit('openLink', entry.link)"
          >
            Open original
          </a>
          <button class="btn" @click="emit('toggleRead')">
            Mark {{ entry.is_read ? "unread" : "read" }}
          </button>
        </div>
      </header>
      <!-- content_html is sanitized with ammonia in the Rust backend before storage -->
      <article class="content" v-html="entry.content_html"></article>
    </template>
    <p v-else class="placeholder">Select an entry to read.</p>
  </section>
</template>

<style scoped>
.reading-pane {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.pane-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 1rem;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #ddd;
  flex-shrink: 0;
}
.pane-title {
  margin: 0 0 0.25rem;
  font-size: 1.1rem;
}
.pane-sub {
  margin: 0;
  color: #777;
  font-size: 0.85rem;
}
.pane-actions {
  display: flex;
  gap: 0.5rem;
  flex-shrink: 0;
}
.btn {
  border: 1px solid #ccc;
  background: #f5f5f5;
  padding: 0.35rem 0.7rem;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.85rem;
  color: inherit;
  text-decoration: none;
}
.btn:hover {
  background: #eaeaea;
}
.content {
  padding: 1rem 1.25rem;
  overflow-y: auto;
  line-height: 1.6;
  flex: 1;
  min-height: 0;
}
.content :deep(img) {
  max-width: 100%;
}
.content :deep(a) {
  color: #2d7ff9;
}
.placeholder {
  margin: auto;
  color: #888;
  font-style: italic;
}
</style>
