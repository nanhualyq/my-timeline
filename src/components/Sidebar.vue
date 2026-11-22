<script setup lang="ts">
import { computed, ref } from "vue";
import {
  addFeed,
  createFolder,
  deleteFolder,
  exportOpml,
  importOpml,
  renameFolder,
  type Feed,
  type Folder,
  type Selection,
} from "../api";
import RowMenu from "./RowMenu.vue";
import { open, save } from "@tauri-apps/plugin-dialog";

const props = defineProps<{
  feeds: Feed[];
  folders: Folder[];
  selection: Selection;
  busy: boolean;
}>();

const emit = defineEmits<{
  select: [selection: Selection];
  changed: [];
  remove: [feedId: number];
  refreshOne: [feedId: number];
  refreshFolder: [folderId: number];
  edit: [feed: Feed];
}>();

const newUrl = ref("");
const addError = ref<string | null>(null);
const actionError = ref<string | null>(null);
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

/** Run a folder action, surface errors, then ask the app to reload. */
async function runAction(fn: () => Promise<unknown>) {
  actionError.value = null;
  try {
    await fn();
    emit("changed");
  } catch (e) {
    actionError.value = String(e);
  }
}

function createFolderAction() {
  const name = prompt("Folder name");
  if (name === null) return;
  void runAction(() => createFolder(name));
}

function renameFolderAction(folder: Folder) {
  const name = prompt("Rename folder", folder.name);
  if (name === null || name.trim() === folder.name) return;
  void runAction(() => renameFolder(folder.id, name));
}

function confirmDeleteFolder(folder: Folder) {
  if (
    confirm(`Delete folder "${folder.name}"? Its feeds move to Uncategorized.`)
  ) {
    void runAction(() => deleteFolder(folder.id));
  }
}

function onFolderMenu(key: string, folder: Folder) {
  if (key === "refresh") emit("refreshFolder", folder.id);
  else if (key === "rename") renameFolderAction(folder);
  else if (key === "delete") confirmDeleteFolder(folder);
}

function onFeedMenu(key: string, feed: Feed) {
  if (key === "edit") emit("edit", feed);
  else if (key === "refresh") emit("refreshOne", feed.id);
  else if (key === "remove") confirmRemove(feed);
}

async function doImport() {
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: "OPML", extensions: ["opml", "xml"] }],
    });
    if (typeof path !== "string") return;
    actionError.value = null;
    const outcome = await importOpml(path);
    alert(
      `Import complete: ${outcome.feeds_added} added, ` +
        `${outcome.feeds_skipped} skipped, ` +
        `${outcome.folders_created} folder(s) created`,
    );
    emit("changed");
  } catch (e) {
    actionError.value = String(e);
  }
}

async function doExport() {
  try {
    const path = await save({
      defaultPath: "subscriptions.opml",
      filters: [{ name: "OPML", extensions: ["opml"] }],
    });
    if (!path) return;
    actionError.value = null;
    const outcome = await exportOpml(path);
    alert(`Exported ${outcome.feeds_exported} feeds`);
  } catch (e) {
    actionError.value = String(e);
  }
}

const uncategorized = computed(() =>
  props.feeds.filter((f) => f.folder_id === null),
);

const folderMenuItems = [
  { key: "refresh", label: "Refresh folder" },
  { key: "rename", label: "Rename" },
  { key: "delete", label: "Delete", danger: true },
];

const feedMenuItems = [
  { key: "edit", label: "Edit\u2026" },
  { key: "refresh", label: "Refresh" },
  { key: "remove", label: "Remove", danger: true },
];

type SidebarItem =
  | { key: string; type: "label"; label: string }
  | { key: string; type: "folder"; folder: Folder }
  | { key: string; type: "feed"; feed: Feed; nested: boolean };

/** Linearized sidebar rows: global views, folder headers with their feeds
 * nested underneath, then the uncategorized group. */
const items = computed<SidebarItem[]>(() => {
  const out: SidebarItem[] = [];
  props.folders.forEach((folder) => {
    out.push({ key: `folder-${folder.id}`, type: "folder", folder });
    for (const feed of folder.feeds) {
      out.push({ key: `feed-${feed.id}`, type: "feed", feed, nested: true });
    }
  });
  if (uncategorized.value.length > 0) {
    out.push({ key: "label-uncategorized", type: "label", label: "Uncategorized" });
    for (const feed of uncategorized.value) {
      out.push({ key: `feed-${feed.id}`, type: "feed", feed, nested: false });
    }
  }
  return out;
});
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
    <p v-if="actionError" class="error">{{ actionError }}</p>

    <div class="opml-actions">
      <button
        type="button"
        class="opml-btn"
        :disabled="busy || addBusy"
        title="Import subscriptions from an OPML file"
        @click="doImport"
      >
        Import OPML
      </button>
      <button
        type="button"
        class="opml-btn"
        :disabled="busy || addBusy"
        title="Export all subscriptions to an OPML file"
        @click="doExport"
      >
        Export OPML
      </button>
    </div>

    <nav class="feed-list">
      <button
        class="feed-item global"
        :class="{ active: selection.kind === 'all' }"
        @click="emit('select', { kind: 'all' })"
      >
        <span class="feed-title">All entries</span>
      </button>
      <button
        class="feed-item global"
        :class="{ active: selection.kind === 'starred' }"
        @click="emit('select', { kind: 'starred' })"
      >
        <span class="feed-title">&#9733; Starred</span>
      </button>

      <div class="group-header">
        <span>Folders</span>
        <button
          class="icon-btn"
          title="New folder"
          :disabled="busy"
          @click="createFolderAction"
        >
          +
        </button>
      </div>

      <template v-for="item in items" :key="item.key">
        <div v-if="item.type === 'label'" class="group-header">
          {{ item.label }}
        </div>

        <div
          v-else-if="item.type === 'folder'"
          class="folder-row"
          :class="{
            active:
              selection.kind === 'folder' && selection.id === item.folder.id,
          }"
        >
          <button
            class="feed-item"
            @click="emit('select', { kind: 'folder', id: item.folder.id })"
          >
            <span class="feed-title" :title="item.folder.name">
              {{ item.folder.name }}
            </span>
            <span
              v-if="item.folder.unread_count > 0"
              class="unread-badge"
            >
              {{ item.folder.unread_count }}
            </span>
          </button>
          <RowMenu
            :items="folderMenuItems"
            trigger-title="Folder actions"
            :disabled="busy"
            @select="(key) => onFolderMenu(key, item.folder)"
          />
        </div>

        <div
          v-else
          class="feed-row"
          :class="{
            active: selection.kind === 'feed' && selection.id === item.feed.id,
            nested: item.nested,
          }"
        >
          <button
            class="feed-item"
            @click="emit('select', { kind: 'feed', id: item.feed.id })"
          >
            <span class="feed-title" :title="item.feed.url">
              {{ item.feed.title }}
            </span>
            <span v-if="item.feed.unread_count > 0" class="unread-badge">
              {{ item.feed.unread_count }}
            </span>
          </button>
          <RowMenu
            :items="feedMenuItems"
            trigger-title="Feed actions"
            :disabled="busy"
            @select="(key) => onFeedMenu(key, item.feed)"
          />
          <span
            v-if="item.feed.last_error"
            class="error-dot"
            :title="item.feed.last_error"
          ></span>
        </div>
      </template>
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
  min-height: 0;
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
.opml-actions {
  display: flex;
  gap: 0.4rem;
}
.opml-btn {
  flex: 1;
  padding: 0.3rem 0.4rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #f5f5f5;
  cursor: pointer;
  font-size: 0.75rem;
}
.opml-btn:hover:not(:disabled) {
  background: #eaeaea;
}
.opml-btn:disabled {
  opacity: 0.6;
  cursor: default;
}
.feed-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  margin-top: 0.5rem;
  padding: 0.15rem 0.5rem;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #888;
}
.folder-row,
.feed-row {
  display: flex;
  align-items: center;
  gap: 2px;
  position: relative;
}
.folder-row.active,
.feed-row.active,
.feed-item.active {
  background: #e3efff;
}
.feed-row.nested .feed-item {
  padding-left: 1.1rem;
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
.feed-item.global .feed-title {
  font-weight: 600;
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
.icon-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 0.9rem;
  padding: 0.15rem 0.3rem;
  border-radius: 4px;
  color: #666;
}
.icon-btn:hover:not(:disabled) {
  background: #ddd;
  color: #000;
}
.icon-btn:disabled {
  opacity: 0.35;
  cursor: default;
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
