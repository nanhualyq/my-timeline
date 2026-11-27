<script setup lang="ts">
import { computed, ref } from "vue";
import {
  deleteFolder,
  markScopeRead,
  renameFolder,
  type Feed,
  type Folder,
  type Selection,
} from "../api";import RowMenu from "./RowMenu.vue";
import { confirm as confirmDialog } from "@tauri-apps/plugin-dialog";

const props = defineProps<{
  feeds: Feed[];
  folders: Folder[];
  selection: Selection;
  busy: boolean;
  /** Total stored entries, for the "All entries" badge. */
  allCount: number;
  /** Total starred entries, for the "Starred" badge. */
  starredCount: number;
  /** Unread entries across the whole library, for the global batch action. */
  unreadCount: number;
  /** True on the one-pane layout, where this component is an overlay drawer
   *  and its controls have to be sized for a fingertip. */
  narrow?: boolean;
}>();

const emit = defineEmits<{
  select: [selection: Selection];
  changed: [];
  remove: [feedId: number];
  refreshOne: [feedId: number];
  refreshFolder: [folderId: number];
  edit: [feed: Feed];
  openSettings: [];
}>();

const actionError = ref<string | null>(null);

/** Folder ids the user has expanded this session. Empty = all collapsed,
 * which is the default at every launch; state is never persisted. */
const expandedFolders = ref(new Set<number>());

function toggleFolder(folderId: number) {
  const next = new Set(expandedFolders.value);
  if (next.has(folderId)) next.delete(folderId);
  else next.add(folderId);
  expandedFolders.value = next;
}

// tauri-plugin-dialog replaces window.confirm with an async wrapper that
// returns a Promise (always truthy), so the native confirm() never blocks
// and a delete would run without waiting for the user. Ask through the
// plugin and await the answer instead.
async function confirmRemove(feed: Feed) {
  const ok = await confirmDialog(`Remove subscription "${feed.title}"?`, {
    title: "Remove subscription",
    kind: "warning",
  });
  if (ok) emit("remove", feed.id);
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

function renameFolderAction(folder: Folder) {
  const name = prompt("Rename folder", folder.name);
  if (name === null || name.trim() === folder.name) return;
  void runAction(() => renameFolder(folder.id, name));
}

async function confirmDeleteFolder(folder: Folder) {
  const ok = await confirmDialog(
    `Delete folder "${folder.name}"? Its feeds move to Uncategorized.`,
    { title: "Delete folder", kind: "warning" },
  );
  if (ok) void runAction(() => deleteFolder(folder.id));
}

function onFolderMenu(key: string, folder: Folder) {
  if (key === "refresh") emit("refreshFolder", folder.id);
  else if (key === "markRead")
    void runAction(() => markScopeRead({ kind: "folder", id: folder.id }));
  else if (key === "rename") renameFolderAction(folder);
  else if (key === "delete") void confirmDeleteFolder(folder);
}

function onFeedMenu(key: string, feed: Feed) {
  if (key === "edit") emit("edit", feed);
  else if (key === "refresh") emit("refreshOne", feed.id);
  else if (key === "markRead")
    void runAction(() => markScopeRead({ kind: "feed", id: feed.id }));
  else if (key === "remove") void confirmRemove(feed);
}

const uncategorized = computed(() =>
  props.feeds.filter((f) => f.folder_id === null),
);

interface MenuItem {
  key: string;
  label: string;
  danger?: boolean;
}

/** The whole-subscription batch action states its own size. It is absent when
 *  nothing is unread, so the menu never offers a no-op. */
function markAllItem(unreadCount: number): MenuItem[] {
  if (unreadCount <= 0) return [];
  return [{ key: "markRead", label: `Mark all as read (${unreadCount})` }];
}

const folderMenuItems = (folder: Folder): MenuItem[] => [
  { key: "refresh", label: "Refresh folder" },
  ...markAllItem(folder.unread_count),
  { key: "rename", label: "Rename" },
  { key: "delete", label: "Delete", danger: true },
];

const feedMenuItems = (feed: Feed): MenuItem[] => [
  { key: "edit", label: "Edit\u2026" },
  { key: "refresh", label: "Refresh" },
  ...markAllItem(feed.unread_count),
  { key: "remove", label: "Remove", danger: true },
];

/** The "All entries" row is the whole library, so its batch item's range is
 *  every unread entry regardless of the selection the list shows. */
const globalMenuItems = (): MenuItem[] => markAllItem(props.unreadCount);

function onGlobalMenu(key: string) {
  if (key === "markRead") void runAction(() => markScopeRead({ kind: "all" }));
}

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
    // Collapsed folders contribute no feed rows: their feeds are not
    // rendered at all (no hidden-but-focusable DOM).
    if (!expandedFolders.value.has(folder.id)) return;
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
    <button
      type="button"
      class="settings-btn"
      title="Settings"
      aria-label="Settings"
      @click="emit('openSettings')"
    >
      <span aria-hidden="true">&#9881;</span>
      <span>Settings</span>
    </button>
    <p v-if="actionError" class="error">{{ actionError }}</p>

    <nav class="feed-list">
      <div class="feed-row">
        <button
          class="feed-item global"
          :class="{ active: selection.kind === 'all' }"
          @click="emit('select', { kind: 'all' })"
        >
          <span class="feed-title">All entries</span>
          <span v-if="allCount > 0" class="unread-badge">{{ allCount }}</span>
        </button>
        <!-- Only rendered when the library holds unread entries: the menu has
             nothing else to offer, so an empty one would be noise. -->
        <RowMenu
          v-if="unreadCount > 0"
          :items="globalMenuItems()"
          trigger-title="All entries actions"
          :touch="props.narrow === true"
          :disabled="busy"
          @select="onGlobalMenu"
        />
      </div>
      <button
        class="feed-item global"
        :class="{ active: selection.kind === 'starred' }"
        @click="emit('select', { kind: 'starred' })"
      >
        <span class="feed-title">&#9733; Starred</span>
        <span v-if="starredCount > 0" class="unread-badge starred">
          {{ starredCount }}
        </span>
      </button>

      <div class="group-header">Folders</div>

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
            type="button"
            class="folder-toggle"
            :aria-expanded="expandedFolders.has(item.folder.id)"
            :aria-label="
              expandedFolders.has(item.folder.id)
                ? 'Collapse folder'
                : 'Expand folder'
            "
            :class="{ 'touch-target': props.narrow }"
            @click.stop="toggleFolder(item.folder.id)"
          >
            <span class="caret" aria-hidden="true">&#9656;</span>
          </button>
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
            :items="folderMenuItems(item.folder)"
            trigger-title="Folder actions"
            :touch="props.narrow === true"
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
            :items="feedMenuItems(item.feed)"
            trigger-title="Feed actions"
            :touch="props.narrow === true"
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
.settings-btn {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  width: 100%;
  padding: 0.4rem 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #f5f5f5;
  cursor: pointer;
  font-size: 0.85rem;
  text-align: left;
}
.settings-btn:active {
  background: #eaeaea;
}
@media (hover: hover) and (pointer: fine) {
  .settings-btn:hover {
    background: #eaeaea;
  }
}
.feed-item:active {
  background: #e6ebf2;
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
.folder-toggle {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.35rem;
  align-self: stretch;
  border: none;
  background: transparent;
  cursor: pointer;
  color: #666;
  border-radius: 4px;
  padding: 0;
}
@media (hover: hover) and (pointer: fine) {
  .folder-toggle:hover {
    background: #eee;
    color: #000;
  }
}
.folder-toggle .caret {
  display: inline-block;
  font-size: 0.7rem;
  transition: transform 0.12s ease;
}
.folder-toggle[aria-expanded="true"] .caret {
  transform: rotate(90deg);
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
@media (hover: hover) and (pointer: fine) {
  .feed-item:hover {
    background: #eee;
  }
}
.feed-item.global .feed-title {
  font-weight: 600;
}
.feed-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* Counts are secondary information: keep them quiet (gray text, no solid
 * pill) so the list stays the focus. Only the starred count is highlighted. */
.unread-badge {
  background: transparent;
  color: #a3a3a3;
  font-size: 0.7rem;
  font-weight: 500;
  flex-shrink: 0;
}
.feed-item:hover .unread-badge:not(.starred),
.feed-item.active .unread-badge:not(.starred) {
  color: #8f8f8f;
}
@media (hover: none) {
  .feed-item:hover .unread-badge:not(.starred) {
    color: #a3a3a3;
  }
}
.unread-badge.starred {
  background: #2d7ff9;
  color: #fff;
  font-size: 0.72rem;
  font-weight: 600;
  padding: 0 0.45rem;
  border-radius: 10px;
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
