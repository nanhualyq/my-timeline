<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Sidebar from "./components/Sidebar.vue";
import EntryList from "./components/EntryList.vue";
import ReadingPane from "./components/ReadingPane.vue";
import FeedEditDialog from "./components/FeedEditDialog.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import {
  listFeeds,
  listFolders,
  listEntries,
  countEntries,
  getEntry,
  refreshAll,
  refreshFeed,
  refreshFolder,
  removeFeed,
  setRead,
  setStarred,
  entryCounts,
  type Cursor,
  type Entry,
  type EntryCounts,
  type EntryListItem,
  type EntryScope,
  type EntryView,
  type Feed,
  type Folder,
  type Selection,
} from "./api";
import { openUrl } from "@tauri-apps/plugin-opener";
import { message } from "@tauri-apps/plugin-dialog";

/** Entries per batch. A constant: the app has no settings persistence, and
 *  batch size is not worth adding one for. */
const PAGE_SIZE = 50;

const feeds = ref<Feed[]>([]);
const folders = ref<Folder[]>([]);
/** Everything loaded so far — grows one batch at a time, never all at once. */
const entries = ref<EntryListItem[]>([]);
/** Where the next batch continues from: null until the first batch lands. */
const after = ref<Cursor | null>(null);
const hasMore = ref(false);
/** A further batch is in flight, so the scroll sentinel does not ask twice. */
const loadingMore = ref(false);
/** Entries matching the current view and scope, for the progress line. */
const total = ref(0);
const selection = ref<Selection>({ kind: "all" });
const selectedEntryId = ref<number | null>(null);
/** Loaded through `getEntry` rather than looked up in `entries`, so paging
 *  away from the open entry does not close the reading pane. */
const selectedEntry = ref<Entry | null>(null);
const view = ref<EntryView>("all");
const counts = ref<EntryCounts>({ all_entries: 0, starred_entries: 0 });
const busy = ref(false);
const loadError = ref<string | null>(null);
const editingFeed = ref<Feed | null>(null);
const settingsOpen = ref(false);

// The two modals are mutually exclusive — never stack them.
function openSettings() {
  editingFeed.value = null;
  settingsOpen.value = true;
}

function startEdit(feed: Feed) {
  settingsOpen.value = false;
  editingFeed.value = feed;
}

const selectedFeed = computed(() => {
  const entry = selectedEntry.value;
  if (!entry) return null;
  return feeds.value.find((f) => f.id === entry.feed_id) ?? null;
});

/** Display titles by feed id, so list rows can name their feed without a
 *  second request. `Feed.title` is already the custom-or-provided title. */
const feedTitles = computed<Record<number, string>>(() => {
  const titles: Record<number, string> = {};
  for (const feed of feeds.value) titles[feed.id] = feed.title;
  return titles;
});

/** The label is omitted where the sidebar already shows the same feed. */
const showEntryFeedLabel = computed(() => selection.value.kind !== "feed");

/** The view the list is actually showing: tabs only apply to the global
 *  views, and the starred view never has an unread filter. */
function activeView(): EntryView {
  if (selection.value.kind === "starred") return "all";
  return isGlobal(selection.value) ? view.value : "all";
}

const displayView = computed<EntryView>(() => activeView());

/** Rows the reader has loaded so far. */
const viewedCount = computed(() => entries.value.length);

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
  const [feedList, folderList, global] = await Promise.all([
    listFeeds(),
    listFolders(),
    entryCounts(),
  ]);
  feeds.value = feedList;
  folders.value = folderList;
  counts.value = global;
}

/** Ask the backend for rows starting at `start`, newest first. */
async function fetchRows(
  start: Cursor | null,
  limit: number,
): Promise<EntryListItem[]> {
  return listEntries(activeView(), scopeOf(selection.value), start, limit);
}

function cursorOfLast(rows: EntryListItem[]): Cursor | null {
  const last = rows[rows.length - 1];
  return last ? { sort: last.sort_key, id: last.id } : null;
}

/** Bumped whenever the list is reset. In-flight responses from the previous
 *  selection are discarded instead of appended — otherwise a fast scope
 *  change would leave rows from the old selection in the new list. */
let generation = 0;

/** Read the rows the reader has loaded, from the newest row down, in one
 *  round trip. Rows are body-free, so even a deep load stays cheap. */
async function fetchWindow(depth: number): Promise<void> {
  const gen = generation;
  const rows = await fetchRows(null, depth);
  if (gen !== generation) return;
  const items = rows.slice(0, depth);
  entries.value = items;
  // One row beyond the window: `hasMore` then costs no extra COUNT.
  hasMore.value = rows.length > depth;
  after.value = cursorOfLast(items);
  total.value = await countEntries(activeView(), scopeOf(selection.value));
}

/** Load the first batch and clear everything else. */
async function loadFirstBatch(): Promise<void> {
  entries.value = [];
  after.value = null;
  hasMore.value = false;
  await fetchWindow(PAGE_SIZE);
}

/** Load the current window in place. Every refresh path calls this, so a
 *  refresh can never move the reader off what they had loaded. */
async function reload() {
  try {
    await refreshCounts();
    // A deleted folder must not leave a dangling selection behind.
    const sel = selection.value;
    if (sel.kind === "folder" && !folders.value.some((f) => f.id === sel.id)) {
      selection.value = { kind: "all" };
    }
    if (entries.value.length === 0) {
      await loadFirstBatch();
    } else {
      await fetchWindow(Math.max(entries.value.length, PAGE_SIZE));
    }
    loadError.value = null;
  } catch (e) {
    loadError.value = String(e);
  }
}

/** Any change of view or scope starts over from the newest batch. */
function startOver() {
  generation += 1;
  entries.value = [];
  after.value = null;
  hasMore.value = false;
  void reload();
}

/** Append the next batch. The scroll sentinel asks; this decides. */
async function loadMore() {
  if (!hasMore.value || loadingMore.value || entries.value.length === 0) return;
  const gen = generation;
  loadingMore.value = true;
  try {
    const rows = await fetchRows(after.value, PAGE_SIZE);
    // The reader changed scope or view while this batch was in flight.
    if (gen !== generation) return;
    const items = rows.slice(0, PAGE_SIZE);
    if (items.length === 0) {
      // Everything after the cursor was deleted: stop rather than spin.
      hasMore.value = false;
    } else {
      entries.value.push(...items);
      after.value = cursorOfLast(items);
      hasMore.value = rows.length > PAGE_SIZE;
    }
    loadError.value = null;
  } catch (e) {
    loadError.value = String(e);
  } finally {
    loadingMore.value = false;
  }
}

function select(sel: Selection) {
  selection.value = sel;
  selectedEntryId.value = null;
  selectedEntry.value = null;
  startOver();
}

function setView(v: EntryView) {
  // The starred view has no unread filter, so tab clicks there are ignored
  // (the tab is hidden anyway); this also keeps "All entries"' tab intact.
  if (selection.value.kind === "starred") return;
  view.value = v;
  // Tabs only apply to the global views; a feed/folder selection falls back
  // to "All entries" (the starred view keeps its scope).
  if (!isGlobal(selection.value)) {
    selection.value = { kind: "all" };
  }
  startOver();
}

async function selectEntry(item: EntryListItem) {
  selectedEntryId.value = item.id;
  // Never leave the previous article on screen under a new title.
  selectedEntry.value = null;
  try {
    const full = await getEntry(item.id);
    // A newer selection, or a cleared pane, already moved on.
    if (selectedEntryId.value !== item.id) return;
    selectedEntry.value = full;
    if (full === null) {
      // The entry is gone (its feed was removed).
      selectedEntryId.value = null;
      return;
    }
    if (!full.is_read) await applyRead(full, true);
  } catch (e) {
    loadError.value = String(e);
  }
}

/** The list row and the open entry are two separate objects after pagination;
 *  every state change goes through here so the two cannot drift apart. */
function patchEntry(
  id: number,
  patch: { is_read?: boolean; is_starred?: boolean },
) {
  const row = entries.value.find((e) => e.id === id);
  if (row) Object.assign(row, patch);
  if (selectedEntry.value?.id === id) Object.assign(selectedEntry.value, patch);
}

async function applyRead(entry: Entry, isRead: boolean) {
  try {
    await setRead(entry.id, isRead);
    patchEntry(entry.id, { is_read: isRead });
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

async function toggleStar(entry: EntryListItem) {
  try {
    const next = !entry.is_starred;
    await setStarred(entry.id, next);
    patchEntry(entry.id, { is_starred: next });
    // Leaving the starred view must drop the entry from its list. The page
    // itself stays put.
    if (selection.value.kind === "starred") {
      await reload();
    }
    // Starring changes the Starred badge, so counts refresh here too.
    await refreshCounts();
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
    // window.alert is overridden by tauri-plugin-dialog into a fire-and-forget
    // call, so await the plugin dialog to keep it in step with the reload.
    await message(`${outcomes.length - failed} feed(s) refreshed, ${failed} failed`);
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
    // Removing a feed cascades to its entries, so the open entry is gone too.
    if (selectedEntry.value?.feed_id === feedId) {
      selectedEntry.value = null;
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
      :all-count="counts.all_entries"
      :starred-count="counts.starred_entries"
      @select="select"
      @changed="reload"
      @remove="doRemove"
      @refresh-one="doRefreshOne"
      @refresh-folder="doRefreshFolder"
      @edit="startEdit"
      @open-settings="openSettings"
    />
    <EntryList
      :entries="entries"
      :selected-entry-id="selectedEntryId"
      :view="displayView"
      :unread-filter-available="selection.kind !== 'starred'"
      :busy="busy"
      :viewed-count="viewedCount"
      :total="total"
      :has-more="hasMore"
      :loading-more="loadingMore"
      :feed-titles="feedTitles"
      :show-feed-label="showEntryFeedLabel"
      @select="selectEntry"
      @toggle-star="toggleStar"
      @refresh-all="doRefreshAll"
      @set-view="setView"
      @load-more="loadMore"
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
    <SettingsDialog
      v-if="settingsOpen"
      :busy="busy"
      @close="settingsOpen = false"
      @changed="reload"
    />
  </main>
</template>

<style>
/* Unscoped: the app owns the whole viewport, so the document itself must
   never scroll — only the individual columns do. */
html,
body {
  margin: 0;
  height: 100%;
  overflow: hidden;
}
#app {
  height: 100%;
}
</style>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: minmax(200px, 260px) minmax(260px, 360px) 1fr;
  /* minmax(0, 1fr): an `auto` implicit row would grow past 100vh with a
     long entry list, scrolling the whole document instead of the column. */
  grid-template-rows: minmax(0, 1fr);
  height: 100vh;
  overflow: hidden;
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
