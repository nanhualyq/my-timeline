<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  moveFeedToFolder,
  setFeedTitle,
  type Feed,
  type Folder,
} from "../api";
import { blockKeyboard } from "../lib/keyboard-blocks";

const props = defineProps<{
  feed: Feed;
  folders: Folder[];
}>();

const emit = defineEmits<{
  close: [];
  saved: [];
}>();

const folderId = ref("");
const title = ref("");
const error = ref<string | null>(null);
const saving = ref(false);

// Draft state resets whenever the dialog is pointed at a (possibly new) feed.
watch(
  () => props.feed,
  (feed) => {
    folderId.value = feed.folder_id === null ? "" : String(feed.folder_id);
    title.value = feed.title;
    error.value = null;
  },
  { immediate: true },
);

async function save() {
  if (saving.value) return;
  saving.value = true;
  error.value = null;
  try {
    const newFolder = folderId.value === "" ? null : Number(folderId.value);
    if (newFolder !== props.feed.folder_id) {
      await moveFeedToFolder(props.feed.id, newFolder);
    }
    // The draft starts as the current display title; only a change is sent.
    // Clearing it sends null so the display reverts to the feed's own title.
    const newTitle = title.value.trim();
    if (newTitle !== props.feed.title) {
      await setFeedTitle(props.feed.id, newTitle === "" ? null : newTitle);
    }
    emit("saved");
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

// While the dialog is open the container must not act on keystrokes meant for
// the fields inside it.
const releaseKeyboard = blockKeyboard();

onMounted(() => document.addEventListener("keydown", onKey));
onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKey);
  releaseKeyboard();
});
</script>

<template>
  <Teleport to="body">
    <div class="dialog-overlay" @click.self="emit('close')">
      <div
        class="dialog"
        role="dialog"
        aria-modal="true"
        :aria-label="`Edit ${feed.title}`"
      >
        <h2 class="dialog-title">Edit feed</h2>

        <label class="field">
          <span class="field-label">Title</span>
          <input
            v-model="title"
            type="text"
            placeholder="Custom title (empty = feed's own)"
          />
        </label>

        <label class="field">
          <span class="field-label">Folder</span>
          <select v-model="folderId">
            <option value="">Uncategorized</option>
            <option v-for="f in folders" :key="f.id" :value="String(f.id)">
              {{ f.name }}
            </option>
          </select>
        </label>

        <dl class="status">
          <dt>URL</dt>
          <dd :title="feed.url">{{ feed.url }}</dd>
          <dt>Last fetched</dt>
          <dd>{{ feed.last_fetched_at ?? "never" }}</dd>
          <dt>Last error</dt>
          <dd>{{ feed.last_error ?? "—" }}</dd>
        </dl>

        <p v-if="error" class="dialog-error">{{ error }}</p>

        <div class="dialog-actions">
          <button type="button" class="secondary" @click="emit('close')">
            Cancel
          </button>
          <button type="button" :disabled="saving" @click="save">Save</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
}
.dialog {
  background: #fff;
  border-radius: 6px;
  padding: 1rem;
  width: min(24rem, calc(100vw - 2rem));
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
}
.dialog-title {
  margin: 0;
  font-size: 1rem;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}
.field-label {
  font-size: 0.75rem;
  font-weight: 600;
  color: #555;
}
.field input,
.field select {
  padding: 0.4rem 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  font-size: 0.85rem;
}
.status {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.15rem 0.6rem;
  margin: 0;
  padding: 0.5rem;
  background: #f7f7f7;
  border-radius: 4px;
  font-size: 0.78rem;
}
.status dt {
  font-weight: 600;
  color: #555;
}
.status dd {
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dialog-error {
  margin: 0;
  color: #c0392b;
  font-size: 0.8rem;
}
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
}
.dialog-actions button {
  padding: 0.4rem 0.9rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #2d7ff9;
  color: #fff;
  cursor: pointer;
}
.dialog-actions button.secondary {
  background: #f5f5f5;
  color: #333;
}
.dialog-actions button:disabled {
  opacity: 0.6;
  cursor: default;
}
</style>
