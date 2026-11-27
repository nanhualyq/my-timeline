<script setup lang="ts">
import { ref, watch } from "vue";
import type { Entry, Feed } from "../api";

const props = defineProps<{
  entry: Entry | null;
  feed: Feed | null;
  /** True on the one-pane layout, where this component is a whole screen and
   *  therefore needs its own way back to the list. */
  narrow?: boolean;
}>();

const emit = defineEmits<{
  toggleRead: [];
  openLink: [url: string];
  /** Narrow layout only: return to the entry list. */
  close: [];
}>();

const contentEl = ref<HTMLElement | null>(null);

/** Scroll the article by whole screens. The keys belong to the container, so
 *  this is a plain method it calls rather than a listener of our own. */
function scrollByScreens(screens: number) {
  const el = contentEl.value;
  if (!el) return;
  el.scrollTop += screens * el.clientHeight;
}

// A new entry starts at its beginning. Post-render, so the measurement is of
// the content that just landed.
watch(
  () => props.entry?.id,
  () => {
    if (contentEl.value) contentEl.value.scrollTop = 0;
  },
  { flush: "post" },
);

defineExpose({ scrollByScreens });
</script>

<template>
  <section class="reading-pane">
    <template v-if="entry">
      <header class="pane-header" :class="{ 'narrow-header': props.narrow }">
        <button
          v-if="props.narrow"
          type="button"
          class="btn back touch-target"
          aria-label="Back to entry list"
          @click="emit('close')"
        >
          <span aria-hidden="true">&#8592;</span>
        </button>
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
          <button
            class="btn"
            :aria-label="entry.is_read ? 'Mark entry unread' : 'Mark entry read'"
            @click="emit('toggleRead')"
          >
            Mark {{ entry.is_read ? "unread" : "read" }}
          </button>
        </div>
      </header>
      <!-- content_html is sanitized with ammonia in the Rust backend before storage -->
      <article ref="contentEl" class="content" v-html="entry.content_html"></article>
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
/* Narrow: the header is a screen header, so it wraps instead of squeezing the
   title to nothing beside the actions. */
.narrow-header {
  flex-wrap: wrap;
  row-gap: 0.4rem;
}
.narrow-header .pane-meta {
  flex: 1 1 12rem;
  min-width: 0;
}
.narrow-header .pane-actions {
  flex-wrap: wrap;
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
.back {
  /* A screen-level control on the narrow layout: it has to be tappable. */
  font-size: 1rem;
  line-height: 1;
}
@media (hover: hover) and (pointer: fine) {
  .btn:hover {
    background: #eaeaea;
  }
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
