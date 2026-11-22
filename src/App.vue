<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Sidebar from "./components/Sidebar.vue";
import EntryList from "./components/EntryList.vue";
import ReadingPane from "./components/ReadingPane.vue";
import {
  listFeeds,
  listEntries,
  refreshAll,
  refreshFeed,
  removeFeed,
  setRead,
  type Entry,
  type Feed,
} from "./api";
import { openUrl } from "@tauri-apps/plugin-opener";

const feeds = ref<Feed[]>([]);
const entries = ref<Entry[]>([]);
const selectedFeedId = ref<number | null>(null);
const selectedEntryId = ref<number | null>(null);
const view = ref<"all" | "unread">("all");
const busy = ref(false);
const loadError = ref<string | null>(null);

const selectedEntry = computed(
  () => entries.value.find((e) => e.id === selectedEntryId.value) ?? null,
);
const selectedFeed = computed(
  () => feeds.value.find((f) => f.id === selectedFeedId.value) ?? null,
);

async function reload() {
  try {
    feeds.value = await listFeeds();
    // Per-feed selection shows that feed; otherwise the active tab applies.
    entries.value = await listEntries(
      selectedFeedId.value === null ? view.value : "all",
      selectedFeedId.value ?? undefined,
    );
    loadError.value = null;
    // Keep the open entry if it still exists; otherwise clear the pane.
    if (
      selectedEntryId.value !== null &&
      !entries.value.some((e) => e.id === selectedEntryId.value)
    ) {
      selectedEntryId.value = null;
    }
  } catch (e) {
    loadError.value = String(e);
  }
}

function selectFeed(feedId: number | null) {
  selectedFeedId.value = feedId;
  selectedEntryId.value = null;
  void reload();
}

function setView(v: "all" | "unread") {
  view.value = v;
  // Tab only applies to the global list; feed selection has its own scope.
  if (selectedFeedId.value !== null) {
    selectedFeedId.value = null;
  }
  void reload();
}

function selectEntry(entry: Entry) {
  selectedEntryId.value = entry.id;
  if (!entry.is_read) {
    void applyRead(entry, true);
  }
}

async function applyRead(entry: Entry, isRead: boolean) {
  try {
    await setRead(entry.id, isRead);
    entry.is_read = isRead;
    // Refresh counts in the sidebar without resetting selection.
    feeds.value = await listFeeds();
  } catch (e) {
    loadError.value = String(e);
  }
}

function toggleRead() {
  const entry = selectedEntry.value;
  if (entry) void applyRead(entry, !entry.is_read);
}

async function doRefreshAll() {
  busy.value = true;
  try {
    await refreshAll();
    await reload();
  } catch (e) {
    loadError.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function doRefreshOne(feedId: number) {
  busy.value = true;
  try {
    await refreshFeed(feedId);
    await reload();
  } catch (e) {
    loadError.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function doRemove(feedId: number) {
  try {
    await removeFeed(feedId);
    if (selectedFeedId.value === feedId) {
      selectedFeedId.value = null;
      selectedEntryId.value = null;
    }
    await reload();
  } catch (e) {
    loadError.value = String(e);
  }
}

async function openLink(url: string) {
  try {
    await openUrl(url);
  } catch {
    // Fallback: nothing sensible to do if the opener plugin fails.
  }
}

onMounted(reload);
</script>

<template>
  <main class="layout">
    <p v-if="loadError" class="global-error">{{ loadError }}</p>
    <Sidebar
      :feeds="feeds"
      :selected-feed-id="selectedFeedId"
      :busy="busy"
      @select="selectFeed"
      @changed="reload"
      @remove="doRemove"
      @refresh-one="doRefreshOne"
    />
    <EntryList
      :entries="entries"
      :selected-entry-id="selectedEntryId"
      :view="view"
      :busy="busy"
      @select="selectEntry"
      @refresh-all="doRefreshAll"
      @set-view="setView"
    />
    <ReadingPane
      :entry="selectedEntry"
      :feed="selectedFeed"
      @toggle-read="toggleRead"
      @open-link="openLink"
    />
  </main>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: minmax(200px, 260px) minmax(260px, 360px) 1fr;
  height: 100vh;
  position: relative;
}
.global-error {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  z-index: 10;
  margin: 0;
  padding: 0.4rem 0.75rem;
  background: #fdecea;
  color: #c0392b;
  border-bottom: 1px solid #f5c6c0;
  font-size: 0.85rem;
}
</style>
