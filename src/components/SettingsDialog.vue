<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  addFeed,
  createFolder,
  exportOpml,
  exportSyncDocument,
  importOpml,
  importSyncDocument,
  loadSyncConfig,
  saveSyncConfig,
  syncNow,
  syncStatus,
  testSyncConnection,
} from "../api";
import type {
  SyncConfig,
  SyncImportSummary,
  SyncStatus,
} from "../api";
import { blockKeyboard } from "../lib/keyboard-blocks";
import {
  pickOpmlDestination,
  pickOpmlText,
  writeOpmlText,
} from "../lib/opml-file";
import {
  pickSyncDestination,
  pickSyncText,
  writeSyncText,
} from "../lib/sync-file";

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

// --- Sync document import/export ---
const syncError = ref<string | null>(null);

/** One sentence on what an import changed, so the reader can see it did something. */
function describe(summary: SyncImportSummary): string {
  const parts: string[] = [];
  if (summary.feeds_added) parts.push(`${summary.feeds_added} feed(s) added`);
  if (summary.feeds_changed) parts.push(`${summary.feeds_changed} feed(s) updated`);
  if (summary.feeds_removed) parts.push(`${summary.feeds_removed} feed(s) removed`);
  if (summary.folders_added) parts.push(`${summary.folders_added} folder(s) added`);
  if (summary.folders_changed) parts.push(`${summary.folders_changed} folder(s) renamed`);
  if (summary.folders_removed) parts.push(`${summary.folders_removed} folder(s) removed`);
  if (summary.stars_applied) parts.push(`${summary.stars_applied} star(s)`);
  if (summary.read_marks_applied) {
    parts.push(`${summary.read_marks_applied} read mark(s)`);
  }
  if (summary.marks_recorded_for_later) {
    parts.push(
      `${summary.marks_recorded_for_later} mark(s) waiting for their feed`,
    );
  }
  return parts.length
    ? `Sync import complete: ${parts.join(", ")}`
    : "Sync import complete: nothing needed changing";
}

async function doSyncImport() {
  syncError.value = null;
  notice.value = null;
  try {
    const text = await pickSyncText();
    if (text === null) return;
    const summary = await importSyncDocument(text);
    notice.value = describe(summary);
    emit("changed");
  } catch (e) {
    syncError.value = String(e);
  }
}

async function doSyncExport() {
  syncError.value = null;
  notice.value = null;
  try {
    const destination = await pickSyncDestination();
    if (!destination) return;
    const outcome = await exportSyncDocument();
    await writeSyncText(destination, outcome.text);
    notice.value =
      `Exported ${outcome.feeds_exported} feeds, ` +
      `${outcome.folders_exported} folder(s), ${outcome.marks_exported} mark(s)`;
  } catch (e) {
    syncError.value = String(e);
  }
}

// --- WebDAV connection ------------------------------------------------
const config = ref<SyncConfig>({
  url: "",
  path: "",
  username: "",
  password: "",
  accept_invalid_certs: false,
});
const syncBusy = ref(false);
const syncConfigError = ref<string | null>(null);
const syncing = ref(false);
const lastStatus = ref<SyncStatus | null>(null);

/** The status line: when the last sync ran, what it did, and why it failed. */
const statusLine = computed(() => {
  const status = lastStatus.value;
  if (status === null) return null;
  const when = new Date(status.ran_at).toLocaleString();
  const prefix =
    status.outcome === "failed" ? `Last sync ${when} failed` : `Last sync ${when}`;
  return `${prefix}: ${status.detail}`;
});

const configured = computed(() => config.value.url.trim() !== "");

async function loadSync() {
  try {
    const saved = await loadSyncConfig();
    if (saved) config.value = saved;
    lastStatus.value = await syncStatus();
  } catch (e) {
    syncConfigError.value = String(e);
  }
}

async function saveSync() {
  if (syncBusy.value || !configured.value) return;
  syncBusy.value = true;
  syncConfigError.value = null;
  notice.value = null;
  try {
    await saveSyncConfig({ ...config.value });
    notice.value = "Sync connection saved";
  } catch (e) {
    syncConfigError.value = String(e);
  } finally {
    syncBusy.value = false;
  }
}

async function testConnection() {
  if (syncBusy.value || !configured.value) return;
  syncBusy.value = true;
  syncConfigError.value = null;
  notice.value = null;
  try {
    // Save first: the backend tests the saved connection, so an unsaved edit
    // would be tested against the wrong settings.
    await saveSyncConfig({ ...config.value });
    const result = await testSyncConnection();
    notice.value = result.document_exists
      ? "Connection works; the document exists"
      : "Connection works; the document will be created on the first sync";
  } catch (e) {
    syncConfigError.value = String(e);
  } finally {
    syncBusy.value = false;
  }
}

async function runSync() {
  if (syncing.value || !configured.value) return;
  syncing.value = true;
  syncConfigError.value = null;
  notice.value = null;
  try {
    await saveSyncConfig({ ...config.value });
    lastStatus.value = await syncNow(true);
    if (lastStatus.value.outcome === "synced" || lastStatus.value.outcome === "created") {
      emit("changed");
    }
  } catch (e) {
    syncConfigError.value = String(e);
  } finally {
    syncing.value = false;
  }
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

// While the dialog is open the container must not act on keystrokes meant for
// the fields inside it.
const releaseKeyboard = blockKeyboard();

onMounted(() => {
  document.addEventListener("keydown", onKey);
  void loadSync();
});
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

        <section class="section">
          <h3 class="section-title">Sync</h3>
          <div class="row sync">
            <button
              type="button"
              :disabled="busy"
              title="Merge a sync document exported from another device"
              @click="doSyncImport"
            >
              Import Sync Document
            </button>
            <button
              type="button"
              :disabled="busy"
              title="Write this library's shared state to a sync document"
              @click="doSyncExport"
            >
              Export Sync Document
            </button>
          </div>
          <p v-if="syncError" class="dialog-error">{{ syncError }}</p>
        </section>

        <section class="section">
          <h3 class="section-title">WebDAV sync</h3>
          <form class="sync-connection" @submit.prevent="saveSync">
            <input
              v-model="config.url"
              type="url"
              placeholder="Server URL — https://dav.example/dav"
              :disabled="busy || syncBusy || syncing"
            />
            <input
              v-model="config.path"
              type="text"
              placeholder="Remote path — my-timeline-sync.json"
              :disabled="busy || syncBusy || syncing"
            />
            <input
              v-model="config.username"
              type="text"
              placeholder="Username"
              :disabled="busy || syncBusy || syncing"
            />
            <input
              v-model="config.password"
              type="password"
              placeholder="Password"
              :disabled="busy || syncBusy || syncing"
            />
            <label class="checkbox">
              <input
                v-model="config.accept_invalid_certs"
                type="checkbox"
                :disabled="busy || syncBusy || syncing"
              />
              Accept an invalid certificate (self-signed server)
            </label>
            <div class="row sync-actions">
              <button type="submit" :disabled="busy || syncBusy || syncing || !configured">
                Save
              </button>
              <button
                type="button"
                :disabled="busy || syncBusy || syncing || !configured"
                title="Check the server, the credentials, and whether the document exists"
                @click="testConnection"
              >
                Test
              </button>
              <button
                type="button"
                :disabled="busy || syncBusy || syncing || !configured"
                title="Sync now, ignoring the focus throttle"
                @click="runSync"
              >
                {{ syncing ? "Syncing…" : "Sync now" }}
              </button>
            </div>
          </form>
          <p v-if="statusLine" class="dialog-status">{{ statusLine }}</p>
          <p v-if="syncConfigError" class="dialog-error">{{ syncConfigError }}</p>
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
.sync-connection {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}
.sync-connection input:not([type="checkbox"]) {
  padding: 0.4rem 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  font-size: 0.85rem;
}
.sync-connection .checkbox {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.8rem;
  color: #444;
}
.row.sync-actions button {
  flex: 1;
}
.dialog-status {
  margin: 0;
  font-size: 0.8rem;
  color: #444;
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
