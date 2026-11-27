<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { addFeed, createFolder, exportOpml, importOpml } from "../api";
import { blockKeyboard } from "../lib/keyboard-blocks";
import {
  pickOpmlDestination,
  pickOpmlText,
  writeOpmlText,
} from "../lib/opml-file";

defineProps<{
  /** True while a background refresh runs; disables the actions below. */
  busy: boolean;
}>();

const emit = defineEmits<{
  close: [];
  changed: [];
}>();

/** Success/outcome messages (add feed, folder created, OPML summaries). */
const notice = ref<string | null>(null);

// --- Add feed (moved verbatim from Sidebar.vue) ---
const newUrl = ref("");
const addError = ref<string | null>(null);
const addBusy = ref(false);

async function submitAdd() {
  const url = newUrl.value.trim();
  if (!url || addBusy.value) return;
  addBusy.value = true;
  addError.value = null;
  notice.value = null;
  try {
    const feed = await addFeed(url);
    newUrl.value = "";
    notice.value = `Added "${feed.title}"`;
    emit("changed");
  } catch (e) {
    addError.value = String(e);
  } finally {
    addBusy.value = false;
  }
}

// --- Create folder (was prompt() in Sidebar.vue) ---
const folderName = ref("");
const folderError = ref<string | null>(null);
const folderBusy = ref(false);

async function submitFolder() {
  const name = folderName.value.trim();
  if (folderBusy.value) return;
  folderError.value = null;
  notice.value = null;
  if (name === "") {
    folderError.value = "Folder name cannot be empty";
    return;
  }
  folderBusy.value = true;
  try {
    const folder = await createFolder(name);
    folderName.value = "";
    notice.value = `Created folder "${folder.name}"`;
    emit("changed");
  } catch (e) {
    folderError.value = String(e);
  } finally {
    folderBusy.value = false;
  }
}

// --- OPML import/export (alert() replaced by the inline notice) ---
const opmlError = ref<string | null>(null);

async function doImport() {
  opmlError.value = null;
  notice.value = null;
  try {
    // The picker returns a path on desktop and a document reference on
    // Android; reading through it is the same call either way.
    const xml = await pickOpmlText();
    if (xml === null) return;
    const outcome = await importOpml(xml);
    notice.value =
      `Import complete: ${outcome.feeds_added} added, ` +
      `${outcome.feeds_skipped} skipped, ` +
      `${outcome.folders_created} folder(s) created`;
    emit("changed");
  } catch (e) {
    opmlError.value = String(e);
  }
}

async function doExport() {
  opmlError.value = null;
  notice.value = null;
  try {
    const destination = await pickOpmlDestination();
    if (!destination) return;
    const outcome = await exportOpml();
    await writeOpmlText(destination, outcome.xml);
    notice.value = `Exported ${outcome.feeds_exported} feeds`;
  } catch (e) {
    opmlError.value = String(e);
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
      <div class="dialog" role="dialog" aria-modal="true" aria-label="Settings">
        <h2 class="dialog-title">Settings</h2>

        <p v-if="notice" class="dialog-notice">{{ notice }}</p>

        <section class="section">
          <h3 class="section-title">Add feed</h3>
          <form class="row" @submit.prevent="submitAdd">
            <input
              v-model="newUrl"
              type="url"
              placeholder="Feed URL…"
              :disabled="busy || addBusy"
            />
            <button type="submit" :disabled="busy || addBusy || !newUrl.trim()">
              Add
            </button>
          </form>
          <p v-if="addError" class="dialog-error">{{ addError }}</p>
        </section>

        <section class="section">
          <h3 class="section-title">New folder</h3>
          <form class="row" @submit.prevent="submitFolder">
            <input
              v-model="folderName"
              type="text"
              placeholder="Folder name"
              :disabled="busy || folderBusy"
            />
            <button type="submit" :disabled="busy || folderBusy">
              Create
            </button>
          </form>
          <p v-if="folderError" class="dialog-error">{{ folderError }}</p>
        </section>

        <section class="section">
          <h3 class="section-title">OPML</h3>
          <div class="row opml">
            <button
              type="button"
              :disabled="busy"
              title="Import subscriptions from an OPML file"
              @click="doImport"
            >
              Import OPML
            </button>
            <button
              type="button"
              :disabled="busy"
              title="Export all subscriptions to an OPML file"
              @click="doExport"
            >
              Export OPML
            </button>
          </div>
          <p v-if="opmlError" class="dialog-error">{{ opmlError }}</p>
        </section>

        <div class="dialog-actions">
          <button type="button" class="secondary" @click="emit('close')">
            Close
          </button>
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
  /* Phone-sized: the dialog must never be pushed past the viewport edges by
     its own height or by a raised on-screen keyboard. */
  padding: 1rem;
  box-sizing: border-box;
}
.dialog {
  background: #fff;
  border-radius: 6px;
  padding: 1rem;
  width: min(26rem, calc(100vw - 2rem));
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  /* Tall content scrolls inside the dialog instead of clipping its actions. */
  max-height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}
.dialog-title {
  margin: 0;
  font-size: 1rem;
}
.section {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}
.section-title {
  margin: 0;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #888;
}
.row {
  display: flex;
  gap: 0.4rem;
}
.row input {
  flex: 1;
  min-width: 0;
  padding: 0.4rem 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  font-size: 0.85rem;
}
.row button {
  padding: 0.4rem 0.7rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #f5f5f5;
  cursor: pointer;
  font-size: 0.85rem;
}
.row.opml button {
  flex: 1;
}
.row button:disabled {
  opacity: 0.6;
  cursor: default;
}
.dialog-notice {
  margin: 0;
  padding: 0.35rem 0.5rem;
  background: #eaf7ee;
  border: 1px solid #c3e6cb;
  border-radius: 4px;
  color: #1e7e34;
  font-size: 0.8rem;
}
.dialog-error {
  margin: 0;
  color: #c0392b;
  font-size: 0.8rem;
}
.dialog-actions {
  display: flex;
  justify-content: flex-end;
}
.dialog-actions button {
  padding: 0.4rem 0.9rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #f5f5f5;
  color: #333;
  cursor: pointer;
}
</style>
