<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Sidebar from "./components/Sidebar.vue";
import EntryList from "./components/EntryList.vue";
import ReadingPane from "./components/ReadingPane.vue";
import FeedEditDialog from "./components/FeedEditDialog.vue";
import {
  listFeeds,
  listFolders,
  listEntries,
  refreshAll,
  refreshFeed,
  refreshFolder,
  removeFeed,
  setRead,
  setStarred,
  type Entry,
  type EntryScope,
  type EntryView,
  type Feed,
  type Folder,
  type Selection,
} from "./api";
import { openUrl } from "@tauri-apps/plugin-opener";

const feeds = ref<Feed[]>([]);
const folders = ref<Folder[]>([]);
const entries = ref<Entry[]>([]);
const selection = ref<Selection>({ kind: "all" });
const selectedEntryId = ref<number | null>(null);
const view = ref<EntryView>("all");
const busy = ref(false);
const loadError = ref<string | null>(null);
const editingFeed = ref<Feed | null>(null);

const selectedEntry = computed(
  () => entries.value.find((e) => e.id === selectedEntryId.value) ?? null,
);
// The reading pane names the entry's own feed, so it works in every view.
const selectedFeed = computed(() => {
  const entry = selectedEntry.value;
  if (!entry) return null;
  return feeds.value.find((f) => f.id === entry.feed_id) ?? null;
});

function scopeOf(sel: Selection): EntryScope {
  switch (sel.kind) {
    case "all":
      return { kind: "all" };
    case "starred":
      return { kind: "starred" };
    case "folder":
      return { kind: "folder", id: sel.id };
    case "feed":
      return { kind: "feed", id: sel.id };
  }
}

function isGlobal(sel: Selection): boolean {
  return sel.kind === "all" || sel.kind === "starred";
}

async function refreshCounts() {
  feeds.value = await listFeeds();
  folders.value = await listFolders();
}

async function reload() {
  try {
    await refreshCounts();
    // A deleted folder must not leave a dangling selection behind.
    const sel = selection.value;
    if (sel.kind === "folder" && !folders.value.some((f) => f.id === sel.id)) {
      selection.value = { kind: "all" };
    }
    // The all/unread tabs apply to the global views; feed and folder
    // selections always show that scope's full list (pre-existing behavior).
    const activeView: EntryView = isGlobal(selection.value) ? view.value : "all";
    entries.value = await listEntries(activeView, scopeOf(selection.value));
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

function select(sel: Selection) {
  selection.value = sel;
  selectedEntryId.value = null;
  void reload();
}

function setView(v: EntryView) {
  view.value = v;
  // Tabs only apply to the global views; a feed/folder selection falls back
  // to "All entries" (the starred view keeps its scope).
  if (!isGlobal(selection.value)) {
    selection.value = { kind: "all" };
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
    await refreshCounts();
  } catch (e) {
    loadError.value = String(e);
  }
}

function toggleRead() {
  const entry = selectedEntry.value;
  if (entry) void applyRead(entry, !entry.is_read);
}

async function toggleStar(entry: Entry) {
  try {
    const next = !entry.is_starred;
    await setStarred(entry.id, next);
    entry.is_starred = next;
    // Leaving the starred view must drop the entry from its list.
    if (selection.value.kind === "starred") {
      entries.value = await listEntries(view.value, { kind: "starred" });
      if (
        selectedEntryId.value !== null &&
        !entries.value.some((e) => e.id === selectedEntryId.value)
      ) {
        selectedEntryId.value = null;
      }
    }
  } catch (e) {
    loadError.value = String(e);
  }
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

async function doRefreshFolder(folderId: number) {
  busy.value = true;
  try {
    const outcomes = await refreshFolder(folderId);
    const failed = outcomes.filter((o) => o.error !== null).length;
    alert(
      `${outcomes.length - failed} feed(s) refreshed, ${failed} failed`,
    );
    await reload();
  } catch (e) {
    loadError.value = String(e);
  } finally {
    busy.value = false;
  }
}

function onFeedSaved() {
  editingFeed.value = null;
  void reload();
}

async function doRemove(feedId: number) {
  try {
    await removeFeed(feedId);
    if (selection.value.kind === "feed" && selection.value.id === feedId) {
      selection.value = { kind: "all" };
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
      :folders="folders"
      :selection="selection"
      :busy="busy"
      @select="select"
      @changed="reload"
      @remove="doRemove"
      @refresh-one="doRefreshOne"
      @refresh-folder="doRefreshFolder"
      @edit="editingFeed = $event"
    />
    <EntryList
      :entries="entries"
      :selected-entry-id="selectedEntryId"
      :view="view"
      :busy="busy"
      @select="selectEntry"
      @toggle-star="toggleStar"
      @refresh-all="doRefreshAll"
      @set-view="setView"
    />
    <ReadingPane
      :entry="selectedEntry"
      :feed="selectedFeed"
      @toggle-read="toggleRead"
      @open-link="openLink"
    />
    <FeedEditDialog
      v-if="editingFeed"
      :feed="editingFeed"
      :folders="folders"
      @close="editingFeed = null"
      @saved="onFeedSaved"
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
