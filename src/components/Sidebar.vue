<script setup lang="ts">
import { ref } from "vue";
import { addFeed, type Feed } from "../api";

const props = defineProps<{
  feeds: Feed[];
  selectedFeedId: number | null;
  busy: boolean;
}>();

const emit = defineEmits<{
  select: [feedId: number | null];
  changed: [];
  remove: [feedId: number];
  refreshOne: [feedId: number];
}>();

const newUrl = ref("");
const addError = ref<string | null>(null);
const addBusy = ref(false);

async function submitAdd() {
  const url = newUrl.value.trim();
  if (!url || addBusy.value) return;
  addBusy.value = true;
  addError.value = null;
  try {
    await addFeed(url);
    newUrl.value = "";
    emit("changed");
  } catch (e) {
    addError.value = String(e);
  } finally {
    addBusy.value = false;
  }
}

function confirmRemove(feed: Feed) {
  if (confirm(`Remove subscription "${feed.title}"?`)) {
    emit("remove", feed.id);
  }
}
</script>

<template>
  <aside class="sidebar">
    <form class="add-form" @submit.prevent="submitAdd">
      <input
        v-model="newUrl"
        type="url"
        placeholder="Feed URL…"
        :disabled="busy || addBusy"
      />
      <button type="submit" :disabled="addBusy || !newUrl.trim()">Add</button>
    </form>
    <p v-if="addError" class="error">{{ addError }}</p>

    <nav class="feed-list">
      <button
        class="feed-item"
        :class="{ active: selectedFeedId === null }"
        @click="emit('select', null)"
      >
        <span class="feed-title">All entries</span>
      </button>

      <div
        v-for="feed in feeds"
        :key="feed.id"
        class="feed-row"
        :class="{ active: selectedFeedId === feed.id }"
      >
        <button class="feed-item" @click="emit('select', feed.id)">
          <span class="feed-title" :title="feed.url">{{ feed.title }}</span>
          <span v-if="feed.unread_count > 0" class="unread-badge">
            {{ feed.unread_count }}
          </span>
        </button>
        <span class="feed-actions">
          <button
            class="icon-btn"
            title="Refresh this feed"
            :disabled="busy"
            @click="emit('refreshOne', feed.id)"
          >
            &#x21bb;
          </button>
          <button
            class="icon-btn"
            title="Remove feed"
            @click="confirmRemove(feed)"
          >
            &times;
          </button>
        </span>
        <span
          v-if="feed.last_error"
          class="error-dot"
          :title="feed.last_error"
        ></span>
      </div>
    </nav>
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.75rem;
  border-right: 1px solid #ddd;
  overflow-y: auto;
}
.add-form {
  display: flex;
  gap: 0.4rem;
}
.add-form input {
  flex: 1;
  min-width: 0;
  padding: 0.4rem 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
}
.add-form button {
  padding: 0.4rem 0.7rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #f5f5f5;
  cursor: pointer;
}
.error {
  color: #c0392b;
  font-size: 0.8rem;
  margin: 0;
}
.feed-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.feed-row {
  display: flex;
  align-items: center;
  gap: 2px;
  position: relative;
}
.feed-row.active,
.feed-item.active {
  background: #e3efff;
}
.feed-item {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.45rem 0.5rem;
  border: none;
  border-radius: 4px;
  background: transparent;
  text-align: left;
  cursor: pointer;
  min-width: 0;
}
.feed-item:hover {
  background: #eee;
}
.feed-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.unread-badge {
  background: #2d7ff9;
  color: #fff;
  font-size: 0.72rem;
  border-radius: 10px;
  padding: 0 0.45rem;
  flex-shrink: 0;
}
.feed-actions {
  display: flex;
  gap: 1px;
}
.icon-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 0.9rem;
  padding: 0.15rem 0.3rem;
  border-radius: 4px;
  color: #666;
}
.icon-btn:hover {
  background: #ddd;
  color: #000;
}
.error-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #e74c3c;
  flex-shrink: 0;
  margin-right: 2px;
}
</style>
