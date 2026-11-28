<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Sidebar from "./components/Sidebar.vue";
import EntryList from "./components/EntryList.vue";
import ReadingPane from "./components/ReadingPane.vue";
import FeedEditDialog from "./components/FeedEditDialog.vue";
import SettingsDialog from "./components/SettingsDialog.vue";
import KeyboardHelp from "./components/KeyboardHelp.vue";
import UndoBar from "./components/UndoBar.vue";
import TouchMenu from "./components/TouchMenu.vue";
import NoticeStack from "./components/NoticeStack.vue";
import RefreshPanel from "./components/RefreshPanel.vue";
import {
  EMPTY_KEY_STATE,
  isEditableTarget,
  isSuppressed,
  resolve,
  toKeystroke,
  tokenOf,
  type Action,
  type KeyState,
} from "./lib/keymap";
import { keyboardBlocked } from "./lib/keyboard-blocks";
import {
  listFeeds,
  listFolders,
  listEntries,
  countEntries,
  getEntry,
  refreshFeed,
  starredFeedIds,
  loadRefreshSettings,
  removeFeed,
  markReadMany,
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
  type RefreshSettings,
  type Selection,
} from "./api";
import { openUrl } from "@tauri-apps/plugin-opener";
import { runBackgroundSync } from "./lib/background-sync";
import {
  createRefreshRunner,
  DEFAULT_CONCURRENCY,
  dueTargets,
  TOUCH_CONCURRENCY,
  type RefreshPanelState,
  type RefreshProgress,
  type RefreshSummary,
  type RefreshTarget,
} from "./lib/refresh";
import { useViewport, useMediaQuery, HOVER_MEDIA_QUERY } from "./lib/viewport";
import { createBackStack, type BackStack } from "./lib/back-stack";
import type { MenuItem } from "./lib/menu";

/** Entries per batch. A constant: the app has no settings persistence, and
 *  batch size is not worth adding one for. */
const PAGE_SIZE = 50;

/** How long a burst of cursor moves keeps accumulating before it is written.
 *  Below a typical key-repeat interval, so releasing the key lands the write
 *  immediately, and above the gap between two deliberate presses. */
const BURST_FLUSH_MS = 120;

/** How long a `g` waits for its second key before it is forgotten. */
const PREFIX_TIMEOUT_MS = 1000;

/** How long a batch mark can be taken back. Long enough to notice a mistake,
 *  short enough that the offer never becomes furniture. */
const UNDO_WINDOW_MS = 3000;

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
/** The list filter. Unread everywhere by default; the Starred view ignores
 *  it and always shows every starred entry. */
const view = ref<EntryView>("unread");
const counts = ref<EntryCounts>({
  all_entries: 0,
  starred_entries: 0,
  unread_entries: 0,
});
const loadError = ref<string | null>(null);
const editingFeed = ref<Feed | null>(null);
/** The row whose long-press menu is open, and where it sits in the list. */
const menuRow = ref<{ entry: EntryListItem; index: number } | null>(null);
const settingsOpen = ref(false);
const helpOpen = ref(false);

/** The published viewport: `layout` decides the columns, `input` decides how
 *  the reader operates them. A tablet in landscape is wide and touch. */
const viewport = useViewport();
const narrow = computed(() => viewport.layout.value === "narrow");
const touch = computed(() => viewport.input.value === "touch");

/** False on a touch screen: nothing there can be hovered, and a reader holding
 *  a phone has no keyboard to press a shortcut on. */
const hoverInput = useMediaQuery(HOVER_MEDIA_QUERY);

/** Narrow-only state. The wide layout ignores both: the third column is
 *  always on screen there, and the sidebar is never an overlay. */
const drawerOpen = ref(false);
const readerOpen = ref(false);

/** The list and the pane lend the container what only they can measure or
 *  scroll; neither decides where the cursor is. */
const listRef = ref<InstanceType<typeof EntryList> | null>(null);
const paneRef = ref<InstanceType<typeof ReadingPane> | null>(null);

/** Where the cursor was, used only when its entry has left the list and a
 *  position has to stand in for an identity. */
const lastCursorIndex = ref(0);

/** Pending keys: a count, and the first key of a sequence waiting for its
 *  second. Reset by `resolve`. */
let keyState: KeyState = EMPTY_KEY_STATE;
let prefixTimer: ReturnType<typeof setTimeout> | null = null;

/** The burst: the row the cursor settled on, and every row it landed on. */
let pendingOpenId: number | null = null;
const pendingReadIds = new Set<number>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;
/** Rows to advance once the batch the cursor walked off has landed. */
let pendingAdvance = 0;

// The two modals are mutually exclusive — never stack them.
function openSettings() {
  editingFeed.value = null;
  settingsOpen.value = true;
}

/* --- Narrow layout navigation -------------------------------------------- */

function openDrawer() {
  if (!narrow.value) return;
  drawerOpen.value = true;
}

function closeDrawer() {
  drawerOpen.value = false;
}

/**
 * Leave the reading screen. The selection is kept, so the row the reader was
 * on stays marked and reopening it is one tap away.
 *
 * The list comes back where the reader left it, except that swiping through
 * articles moves the cursor and the list has to follow: a list still parked
 * three screens above what was just read would be a list that lost its place.
 * `revealEntry` runs post-render, because the list is only measurable once it
 * is on screen again.
 */
function closeReader() {
  readerOpen.value = false;
  const id = selectedEntryId.value;
  if (id === null) return;
  void nextTick(() => listRef.value?.revealEntry(id));
}

function openReader() {
  if (narrow.value) readerOpen.value = true;
}

/**
 * The row's long-press menu: the same two row actions the pointer layout puts
 * on the row itself, offered where a finger cannot hover and where an assistive
 * technology would swallow a horizontal drag.
 */
function openRowMenu(entry: EntryListItem, index: number) {
  menuRow.value = { entry, index };
}

function closeRowMenu() {
  menuRow.value = null;
}

/** Unread rows above the menu's row, which is what its batch item reaches. */
const menuUnreadAbove = computed(() => {
  const row = menuRow.value;
  if (!row) return 0;
  return entries.value.slice(0, row.index).filter((e) => !e.is_read).length;
});

const menuItems = computed<MenuItem[]>(() => {
  const row = menuRow.value;
  if (!row) return [];
  const items: MenuItem[] = [
    {
      key: "star",
      label: row.entry.is_starred ? "Unstar entry" : "Star entry",
    },
  ];
  // Same condition as the visible chip: nothing to mark, nothing offered.
  if (batchActionsAvailable.value && menuUnreadAbove.value > 0) {
    const count = menuUnreadAbove.value;
    items.push({
      key: "mark-above",
      label: `Mark ${count} ${count === 1 ? "entry" : "entries"} above read`,
    });
  }
  return items;
});

function chooseRowMenu(key: string) {
  const row = menuRow.value;
  menuRow.value = null;
  if (!row) return;
  if (key === "star") void toggleStar(row.entry);
  else if (key === "mark-above") markAboveRead(row.index);
}

/**
 * Crossing the breakpoint has to reconcile the narrow-only state, in one
 * place: a drawer has nowhere to overlay in the wide layout, while an open
 * entry is already what the third column shows, so it stays open.
 */
watch(narrow, (isNarrow) => {
  drawerOpen.value = false;
  if (isNarrow) {
    readerOpen.value = selectedEntryId.value !== null;
  }
});

/* --- System back --------------------------------------------------------- */

/**
 * The layers the back action can close, bottom to top: the drawer first (it
 * sits over the list), the reading screen over that, and any blocking dialog
 * on top of everything. The last entry is what back closes first.
 */
const openLayers = computed<string[]>(() => {
  const layers: string[] = [];
  if (narrow.value && drawerOpen.value) layers.push("drawer");
  if (narrow.value && readerOpen.value) layers.push("reader");
  if (menuRow.value) layers.push("menu");
  if (settingsOpen.value || editingFeed.value !== null || helpOpen.value) {
    layers.push("dialog");
  }
  return layers;
});

/** Close exactly one layer, topmost first: what the system back action asks. */
function closeTopLayer() {
  if (settingsOpen.value || editingFeed.value !== null || helpOpen.value) {
    settingsOpen.value = false;
    editingFeed.value = null;
    helpOpen.value = false;
    return;
  }
  if (menuRow.value) {
    menuRow.value = null;
    return;
  }
  if (narrow.value && readerOpen.value) {
    readerOpen.value = false;
    return;
  }
  if (narrow.value && drawerOpen.value) drawerOpen.value = false;
}

/** Created during setup so the first history entry can be pushed as soon as a
 *  layer opens; there is nothing to unwind before that. */
let backStack: BackStack | null = null;
watch(
  openLayers,
  (layers) => backStack?.sync(layers.length),
  { flush: "post" },
);

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

/** Batch marking acts on the rows on screen, so it belongs to lists whose
 *  contents are still to be dealt with: a feed, a folder, or the global unread
 *  view. "All entries" unfiltered is an archive and Starred is a shelf. */
const batchActionsAvailable = computed(() => {
  const sel = selection.value;
  if (sel.kind === "feed" || sel.kind === "folder") return true;
  return sel.kind === "all" && view.value === "unread";
});

/** The view the list is actually showing. The filter applies to whatever is
 *  selected; only the starred shelf is above it. */
function activeView(): EntryView {
  if (selection.value.kind === "starred") return "all";
  return view.value;
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
    // The open entry is a separate object from the list rows, so whatever
    // triggered this reload may have changed it (a whole-feed batch mark from
    // the sidebar, a refresh, or its feed being removed). Re-read that one row
    // rather than letting the reading pane's toggle go stale.
    trackCursor();
    if (selectedEntryId.value !== null) {
      const id = selectedEntryId.value;
      const full = await getEntry(id);
      if (selectedEntryId.value === id) {
        selectedEntry.value = full;
        if (full === null) {
          // The entry no longer exists (its feed was removed): the cursor
          // stands in at the position it held, and the pane loads that row.
          replaceVanishedCursor();
          const replacement = selectedEntryId.value;
          if (replacement !== null) {
            selectedEntry.value = await getEntry(replacement);
          }
        }
      }
    }
    loadError.value = null;
  } catch (e) {
    loadError.value = String(e);
  }
}

/** Any change of view or scope starts over from the newest batch. */
function startOver() {
  // Whatever the cursor did before this keypress belongs to the list that is
  // about to be replaced; write it before that list is gone.
  void flushBurst();
  generation += 1;
  entries.value = [];
  after.value = null;
  hasMore.value = false;
  pendingAdvance = 0;
  void reload();
}

/** Append the next batch. The scroll sentinel asks; this decides. */
async function loadMore() {
  if (!hasMore.value || loadingMore.value || entries.value.length === 0) {
    // Nothing will be appended, so a pending carry-over has nothing to land on.
    pendingAdvance = 0;
    return;
  }
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
      pendingAdvance = 0;
    } else {
      entries.value.push(...items);
      after.value = cursorOfLast(items);
      hasMore.value = rows.length > PAGE_SIZE;
      trackCursor();
      // A move that walked off the old last row continues here.
      settleAdvance();
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
  lastCursorIndex.value = 0;
  // On a phone the list is the only thing on screen after a selection; the
  // drawer that made the selection gets out of the way.
  closeDrawer();
  readerOpen.value = false;
  startOver();
}

function setView(v: EntryView) {
  // The starred view has no unread filter, so tab clicks there are ignored
  // (the tabs are hidden anyway).
  if (selection.value.kind === "starred") return;
  // The filter belongs to the list, not to the global views: choosing All
  // inside a feed keeps that feed selected. The choice carries over to the
  // next selection, so browsing a feed's archive does not reset on every hop.
  view.value = v;
  startOver();
}

async function selectEntry(item: EntryListItem) {
  selectedEntryId.value = item.id;
  // Never leave the previous article on screen under a new title.
  selectedEntry.value = null;
  openReader();
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

async function applyRead(entry: { id: number }, isRead: boolean) {
  try {
    await setRead(entry.id, isRead);
    patchEntry(entry.id, { is_read: isRead });
    // Refresh counts in the sidebar without resetting selection.
    await refreshCounts();
  } catch (e) {
    loadError.value = String(e);
  }
}

/** The reading pane's star control: the open entry, which is a separate object
 *  from its list row. */
function toggleStarOpenEntry() {
  const entry = selectedEntry.value;
  if (entry) void toggleStar(entry);
}

/** A horizontal swipe in the reading pane is a cursor move, so it inherits
 *  marking-on-open, the coalesced write, and paging past the loaded rows. */
function stepEntry(delta: number) {
  void stepCursor(delta);
}

/** Whether the open entry has neighbours, for the reading screen's indication
 *  of what a swipe would do. `hasMore` counts as a next entry: swiping there
 *  loads the batch that holds it. */
const hasPreviousEntry = computed(() => cursorIndex() > 0);
const hasNextEntry = computed(() => {
  const index = cursorIndex();
  if (index < 0) return false;
  return index < entries.value.length - 1 || hasMore.value;
});

function toggleRead() {
  const entry = selectedEntry.value;
  if (entry) void applyRead(entry, !entry.is_read);
}

/**
 * Mark rows read without touching the loaded rows.
 *
 * The difference from `applyBatchRead` is the point: a batch action refills an
 * unread list, while these rows were marked because the cursor moved onto them.
 * Resetting here would move the list under the cursor on every keystroke.
 */
async function applyReadSilently(ids: number[]) {
  if (ids.length === 0) return;
  try {
    await markReadMany(ids);
    for (const id of ids) patchEntry(id, { is_read: true });
    await refreshCounts();
  } catch (e) {
    loadError.value = String(e);
  }
}

function markRenderedRead() {
  void applyBatchRead(
    entries.value.filter((e) => !e.is_read).map((e) => e.id),
  );
}

/** Mark the unread rows above `index`, excluding that row: the list only
 *  appends downwards, so everything above a loaded row is loaded too. */
function markAboveRead(index: number) {
  void applyBatchRead(
    entries.value
      .slice(0, index)
      .filter((e) => !e.is_read)
      .map((e) => e.id),
  );
}

/**
 * The batch mark that can still be taken back, and the timer that retires the
 * offer. One slot, deliberately: a second batch replaces the first rather than
 * stacking, because a three-second window cannot realistically hold two.
 */
const pendingUndo = ref<{ ids: number[] } | null>(null);
let undoTimer: ReturnType<typeof setTimeout> | null = null;

function retireUndo() {
  if (undoTimer !== null) {
    clearTimeout(undoTimer);
    undoTimer = null;
  }
  pendingUndo.value = null;
}

function armUndo(ids: number[]) {
  retireUndo();
  pendingUndo.value = { ids: [...ids] };
  undoTimer = setTimeout(() => {
    undoTimer = null;
    pendingUndo.value = null;
  }, UNDO_WINDOW_MS);
}

/**
 * Take back the last batch mark.
 *
 * Only the entries that batch recorded are written, so anything marked or
 * unmarked by another path in the meantime keeps its state. In an unread list
 * the marked rows have already left the page, so their read state alone would
 * be invisible: the list is refilled instead of patched, which costs the scroll
 * position and shows the reader what came back.
 */
async function undoBatchRead() {
  const pending = pendingUndo.value;
  if (!pending) return;
  retireUndo();
  try {
    await Promise.all(pending.ids.map((id) => setRead(id, false)));
    for (const id of pending.ids) patchEntry(id, { is_read: false });
    if (activeView() === "unread") {
      startOver();
      return;
    }
    await refreshCounts();
  } catch (e) {
    loadError.value = String(e);
  }
}

async function applyBatchRead(ids: number[]) {
  if (ids.length === 0) return;
  try {
    await markReadMany(ids);
    // The client knows exactly which rows it marked, so no refetch is needed
    // for the rows or the open entry.
    for (const id of ids) patchEntry(id, { is_read: true });
    if (activeView() === "unread") {
      // Those rows just left the list; refill the page from the newest batch
      // so the reader never has to scroll to reach the next page.
      startOver();
    }
    await refreshCounts();
    // Armed last, so the offer appears only once the write has landed.
    armUndo(ids);
  } catch (e) {
    loadError.value = String(e);
  }
}

/** Star anything with a star and an id — a list row or the open entry. */
async function toggleStar(entry: { id: number; is_starred: boolean }) {
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

/* --- Refresh -------------------------------------------------------------- */

/** How long a run must last before its progress is worth showing. A two-feed
 *  refresh should not flash a panel. */
const PANEL_DELAY_MS = 400;

/** How long a clean summary stays before it withdraws. */
const SUMMARY_LINGER_MS = 4000;

/** How often the schedule looks for feeds whose content has gone stale. */
const AUTO_TICK_MS = 60_000;

/** The saved schedule. Defaults match the backend's, so a library that has
 *  never saved one behaves like a fresh install. */
const refreshSettings = ref<RefreshSettings>({
  enabled: false,
  interval_minutes: 30,
});

/** The run in flight, as the panel needs it: a counter, the finished feeds, and
 *  the hosts being left alone. Null while nothing runs. */
const refreshProgress = ref<RefreshProgress | null>(null);
/** The last finished run, kept until it is dismissed or replaced. */
const refreshSummary = ref<RefreshSummary | null>(null);
/** Whether the panel is on screen at all. */
const panelOpen = ref(false);
/** True while a run is dispatching, which is what `busy` means to the panes. */
const refreshRunning = ref(false);

const busy = computed(() => refreshRunning.value);

let panelTimer: ReturnType<typeof setTimeout> | null = null;
let summaryTimer: ReturnType<typeof setTimeout> | null = null;
/** A due-eligible check is resolving or running: the schedule and the
 *  foreground return must not both start one. */
let dueCheckRunning = false;

/** When the schedule last started a run, for the Settings dialog to report. */
const lastScheduledAt = ref<string | null>(null);

function clearPanelTimers() {
  if (panelTimer !== null) {
    clearTimeout(panelTimer);
    panelTimer = null;
  }
  if (summaryTimer !== null) {
    clearTimeout(summaryTimer);
    summaryTimer = null;
  }
}

function closePanel() {
  clearPanelTimers();
  panelOpen.value = false;
  refreshSummary.value = null;
  refreshProgress.value = null;
}

/**
 * Every trigger reaches this one runner: the list's action, its keystroke, the
 * pull gesture, the schedule, and the foreground return. It is single-flight
 * and de-duplicates per feed, so the triggers cannot multiply the work.
 */
const refreshRunner = createRefreshRunner({
  refreshFeed,
  concurrency: () => (touch.value ? TOUCH_CONCURRENCY : DEFAULT_CONCURRENCY),
  onStart: (total) => {
    clearPanelTimers();
    refreshRunning.value = true;
    refreshSummary.value = null;
    refreshProgress.value = {
      total,
      done: 0,
      results: [],
      backoffs: [],
      stopping: false,
    };
    // A long run announces itself; a short one only reports its summary.
    panelTimer = setTimeout(() => {
      panelTimer = null;
      panelOpen.value = true;
    }, PANEL_DELAY_MS);
  },
  onProgress: (progress) => {
    refreshProgress.value = progress;
  },
  onDone: (summary) => {
    dueCheckRunning = false;
    refreshRunning.value = false;
    refreshProgress.value = null;
    refreshSummary.value = summary;
    clearPanelTimers();
    // The summary outlives the run, so it is shown even when the progress panel
    // never had time to appear.
    panelOpen.value = true;
    if (summary.failed === 0) {
      summaryTimer = setTimeout(() => {
        summaryTimer = null;
        closePanel();
      }, SUMMARY_LINGER_MS);
    }
    if (summary.ok + summary.failed > 0) void reload();
  },
});

/** What the panel is showing, when it is showing anything. */
const panelState = computed<RefreshPanelState | null>(() => {
  if (refreshProgress.value) return { phase: "running", progress: refreshProgress.value };
  if (refreshSummary.value) return { phase: "done", summary: refreshSummary.value };
  return null;
});

/** The targets a selection's refresh covers. Starred is the only scope that
 *  needs the backend: which feeds hold a starred entry is not a list the
 *  frontend has. */
async function targetsFor(sel: Selection): Promise<RefreshTarget[]> {
  const toTarget = (feed: Feed): RefreshTarget => ({
    feedId: feed.id,
    url: feed.url,
    lastFetchedAt: feed.last_fetched_at,
  });
  switch (sel.kind) {
    case "all":
      return feeds.value.map(toTarget);
    case "feed":
      return feeds.value.filter((feed) => feed.id === sel.id).map(toTarget);
    case "folder":
      return feeds.value.filter((feed) => feed.folder_id === sel.id).map(toTarget);
    case "starred": {
      const ids = await starredFeedIds().catch(() => [] as number[]);
      const starred = new Set(ids);
      return feeds.value.filter((feed) => starred.has(feed.id)).map(toTarget);
    }
  }
}

/**
 * Start a run for a scope. `due` keeps only the feeds whose content is older
 * than the interval, which is what the schedule and the foreground return ask
 * for: refetching a feed fetched a minute ago would only spend the reader's
 * connection. Returns whether a run started.
 */
async function refreshScope(
  sel: Selection,
  options: { due?: boolean } = {},
): Promise<boolean> {
  let targets: RefreshTarget[];
  try {
    targets = await targetsFor(sel);
  } catch (e) {
    loadError.value = String(e);
    return false;
  }
  if (options.due) {
    targets = dueTargets(
      targets,
      refreshSettings.value.interval_minutes,
      Date.now(),
    );
  }
  if (targets.length === 0) return false;
  refreshRunner.start(targets);
  return true;
}

function doRefreshAll() {
  return refreshScope({ kind: "all" });
}

function doRefreshOne(feedId: number) {
  return refreshScope({ kind: "feed", id: feedId });
}

function doRefreshFolder(folderId: number) {
  return refreshScope({ kind: "folder", id: folderId });
}

/** The pull gesture: refresh what the list is showing. */
function refreshCurrentList() {
  return refreshScope(selection.value);
}

function stopRefresh() {
  refreshRunner.cancel();
}

/** Fetch only the feeds the last run failed on. */
async function retryFailedFeeds() {
  const summary = refreshSummary.value;
  if (!summary) return;
  const failed = new Set(
    summary.results.filter((result) => !result.ok).map((result) => result.feedId),
  );
  const targets = feeds.value
    .filter((feed) => failed.has(feed.id))
    .map<RefreshTarget>((feed) => ({
      feedId: feed.id,
      url: feed.url,
      lastFetchedAt: feed.last_fetched_at,
    }));
  if (targets.length === 0) {
    closePanel();
    return;
  }
  refreshRunner.start(targets);
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

/* --- Keyboard ------------------------------------------------------------ */

/** The row the cursor is on. The cursor *is* the open entry: there is no
 *  second piece of state that could point at a different row. */
function cursorRow(): EntryListItem | null {
  const id = selectedEntryId.value;
  if (id === null) return null;
  return entries.value.find((row) => row.id === id) ?? null;
}

function cursorIndex(): number {
  const id = selectedEntryId.value;
  return id === null ? -1 : entries.value.findIndex((row) => row.id === id);
}

/**
 * Open a row because the cursor landed on it.
 *
 * The row is marked read in the list immediately — a keystroke must not wait
 * for a request — while the write itself is coalesced with the rest of the
 * burst. That is what keeps holding `j` from becoming one write per repeat.
 */
function openAtCursor(index: number) {
  const row = entries.value[index];
  if (!row) return;
  lastCursorIndex.value = index;
  if (row.id === selectedEntryId.value) return;
  selectedEntryId.value = row.id;
  // A hardware keyboard on a phone opens the reading screen for the row the
  // cursor lands on, exactly like tapping it does.
  openReader();
  // The previous article stays on screen until the new one lands: blanking the
  // pane on every keystroke of a burst would only flicker. The flush keeps the
  // stale-response guard, so a late reply for an abandoned row is dropped.
  if (!row.is_read) {
    row.is_read = true;
    pendingReadIds.add(row.id);
  }
  pendingOpenId = row.id;
  armFlush();
}

/**
 * Keep the cursor's recorded position current as rows are added above or
 * removed from the list. It never decides that the cursor is gone: only
 * `reload` learns that, by asking the backend for the entry itself.
 */
function trackCursor() {
  const index = cursorIndex();
  if (index >= 0) lastCursorIndex.value = index;
}

/** Stand in for an entry that no longer exists, at the position it held. */
function replaceVanishedCursor() {
  if (entries.value.length === 0) {
    selectedEntryId.value = null;
    selectedEntry.value = null;
    return;
  }
  const fallback = Math.min(lastCursorIndex.value, entries.value.length - 1);
  selectedEntryId.value = entries.value[fallback].id;
  lastCursorIndex.value = fallback;
}

/** Move the cursor by `delta` rows, or stop at the ends. */
async function stepCursor(delta: number) {
  if (entries.value.length === 0) return;
  const from = cursorIndex();
  const base = from < 0 ? (delta > 0 ? -1 : 0) : from;
  const target = base + delta;
  if (target < 0) return;
  const last = entries.value.length - 1;
  if (target > last) {
    // Walked off the loaded rows with more to come: continue on the next batch
    // rather than dropping the keypress.
    if (delta > 0 && hasMore.value) {
      pendingAdvance = 1;
      await loadMore();
      return;
    }
    // A counted move stops at the end; a single step at the end stays put.
    if (Math.abs(delta) > 1) openAtCursor(last);
    return;
  }
  openAtCursor(target);
}

/** `G`: a numbered row when counted, otherwise the end of the loaded rows.
 *
 * One batch per press, deliberately. The list is keyset-paginated, so there is
 * no way to reach a distant last row except by walking every batch between
 * here and there; loading them all on one keypress would leave the cursor
 * parked while hundreds of round trips run. The first press lands on the end
 * the reader can actually see, and each further press extends it.
 */
async function jumpToBottom(row: number | null) {
  if (row !== null) {
    if (entries.value.length === 0) return;
    openAtCursor(Math.min(row - 1, entries.value.length - 1));
    return;
  }
  if (entries.value.length === 0) return;
  const last = entries.value.length - 1;
  if (cursorIndex() !== last) {
    openAtCursor(last);
    return;
  }
  // Already at the end: one more batch, then the new end of the list.
  if (!hasMore.value) return;
  await loadMore();
  if (entries.value.length > last + 1) openAtCursor(entries.value.length - 1);
}

/** Continue a move that walked off the loaded rows. */
function settleAdvance() {
  if (pendingAdvance === 0) return;
  const advance = pendingAdvance;
  pendingAdvance = 0;
  const index = cursorIndex();
  if (index < 0) return;
  const target = Math.min(index + advance, entries.value.length - 1);
  if (target !== index) openAtCursor(target);
}

/** Half a screen of rows, measured by the list. A list that has not been laid
 *  out yet — a keypress before first layout — still has to move somewhere. */
function halfPageSize(): number {
  const visible = listRef.value?.visibleRowCount() ?? 0;
  return Math.max(1, Math.floor((visible > 0 ? visible : 20) / 2));
}

/** Write what a burst of moves did: one content fetch for the row the cursor
 *  settled on, one write for every row it landed on, one count refresh. */
async function flushBurst() {
  if (flushTimer !== null) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }
  const openId = pendingOpenId;
  pendingOpenId = null;
  const ids = [...pendingReadIds];
  pendingReadIds.clear();

  if (openId !== null && openId === selectedEntryId.value) {
    try {
      const full = await getEntry(openId);
      // A newer move already replaced this row.
      if (selectedEntryId.value === openId) selectedEntry.value = full;
    } catch (e) {
      loadError.value = String(e);
    }
  }
  await applyReadSilently(ids);
}

function armFlush() {
  if (flushTimer !== null) clearTimeout(flushTimer);
  flushTimer = setTimeout(() => {
    flushTimer = null;
    void flushBurst();
  }, BURST_FLUSH_MS);
}

/** Run an action that writes right away, once the cursor's burst is written:
 *  a pending mark landing afterwards would undo the action. */
async function afterBurst(fn: () => void | Promise<void>) {
  await flushBurst();
  await fn();
}

function toggleReadAtCursor() {
  const row = cursorRow();
  if (row) return applyRead(row, !row.is_read);
}

function toggleStarAtCursor() {
  const row = cursorRow();
  if (row) return toggleStar(row);
}

function openLinkAtCursor() {
  const row = cursorRow();
  if (row?.link) return openLink(row.link);
}

function runAction(action: Action, count: number | null) {
  const steps = count ?? 1;
  switch (action) {
    case "cursor-down":
      void stepCursor(steps);
      return;
    case "cursor-up":
      void stepCursor(-steps);
      return;
    case "cursor-top":
      if (entries.value.length > 0) openAtCursor(0);
      return;
    case "cursor-bottom":
      void jumpToBottom(count);
      return;
    case "half-page-down":
      void stepCursor(halfPageSize() * steps);
      return;
    case "half-page-up":
      void stepCursor(-halfPageSize() * steps);
      return;
    case "pane-scroll-down":
      paneRef.value?.scrollByScreens(steps);
      return;
    case "pane-scroll-up":
      paneRef.value?.scrollByScreens(-steps);
      return;
    case "toggle-help":
      // The shortcut reference is not offered on a touch layout: there is no
      // keyboard to use it with.
      if (touch.value && !hoverInput.value) return;
      helpOpen.value = true;
      return;
    case "toggle-read":
      void afterBurst(toggleReadAtCursor);
      return;
    case "toggle-star":
      void afterBurst(toggleStarAtCursor);
      return;
    case "open-link":
      void afterBurst(openLinkAtCursor);
      return;
    case "toggle-view":
      // The starred view has no filter; `setView` already ignores it there.
      setView(displayView.value === "unread" ? "all" : "unread");
      return;
    case "mark-rendered":
      void afterBurst(() => markRenderedRead());
      return;
    case "refresh-all":
      void afterBurst(() => void doRefreshAll());
      return;
    case "go-all":
      select({ kind: "all" });
      return;
    case "go-starred":
      select({ kind: "starred" });
      return;
  }
}

function clearPrefixTimer() {
  if (prefixTimer !== null) {
    clearTimeout(prefixTimer);
    prefixTimer = null;
  }
}

/** A `g` that never completes is forgotten, and is never re-read as a command
 *  of its own: nothing is bound to a bare `g`. */
function armPrefixTimer() {
  clearPrefixTimer();
  prefixTimer = setTimeout(() => {
    prefixTimer = null;
    keyState = EMPTY_KEY_STATE;
  }, PREFIX_TIMEOUT_MS);
}

function onKeydown(event: KeyboardEvent) {
  const stroke = toKeystroke(event);
  const editable = isEditableTarget(event.target);

  if (isSuppressed(stroke, { blocked: keyboardBlocked.value, editable })) {
    // The help overlay holds the keyboard like a dialog, so it is also the one
    // blocked surface a key closes.
    if (
      helpOpen.value &&
      !editable &&
      (stroke.key === "Escape" || stroke.key === "?")
    ) {
      event.preventDefault();
      helpOpen.value = false;
    }
    return;
  }

  const token = tokenOf(stroke);
  if (token === null) return;

  const resolution = resolve(keyState, token);
  keyState = resolution.state;

  if (resolution.kind === "pending") {
    armPrefixTimer();
    return;
  }
  clearPrefixTimer();
  if (resolution.kind === "ignored") return;

  // The key is ours, so the webview must not also act on it.
  event.preventDefault();
  runAction(resolution.action, resolution.count);
}

/** A pending write must not wait for the reader to do something else. */
function flushOnLeave() {
  void flushBurst();
}

/* --- Returning to the app ------------------------------------------------- */

/** How long the app must have been hidden for a return to count as a resume.
 *  Opening the app switcher or the notification shade must not fetch. */
const RESUME_REFRESH_MS = 60_000;

let hiddenAt: number | null = null;

/**
 * Refresh what the reader is looking at after the app comes back from the
 * background, but only the feeds whose content went stale while it was away.
 * A return is not a reason to refetch a feed that is already fresh, and it is
 * never a reason for a second run: the guard below keeps a foreground return
 * and a scheduled check from both starting one.
 */
async function dueRefresh(sel: Selection) {
  if (dueCheckRunning) return;
  dueCheckRunning = true;
  const started = await refreshScope(sel, { due: true });
  // The flag is cleared when the run reports its summary; a check that found
  // nothing due has nothing to wait for.
  if (!started) dueCheckRunning = false;
}

/** The scheduled check: only while the app is visible and only for feeds whose
 *  content is older than the interval. Cheap enough to run every minute,
 *  because a tick with nothing due sends no request at all. */
function scheduledCheck() {
  if (!refreshSettings.value.enabled) return;
  if (document.visibilityState === "hidden") return;
  lastScheduledAt.value = new Date().toISOString();
  void dueRefresh({ kind: "all" });
}

/**
 * Android reports backgrounding through the document's visibility. Only the
 * narrow layout refreshes on it: on a desktop window, losing focus means the
 * reader is looking at something else on the same screen.
 */
function onVisibilityChange() {
  if (document.visibilityState === "hidden") {
    hiddenAt = Date.now();
    return;
  }
  const hiddenSince = hiddenAt;
  hiddenAt = null;
  if (!narrow.value) return;
  if (hiddenSince === null) return;
  if (Date.now() - hiddenSince < RESUME_REFRESH_MS) return;
  void dueRefresh(selection.value);
}

/** Sync in the background, then refresh the view if the library changed.
 *  Never surfaces an error: an unconfigured connection and a failed sync are
 *  both reported in Settings, which is where the connection is configured. */
async function syncQuietly(): Promise<void> {
  await runBackgroundSync(reload);
}

/** The window regained focus: a moment worth checking the server. Throttled in
 *  the backend, so extra events cost one channel message. */
function onWindowFocus() {
  void syncQuietly();
}

/** Coming back to a visible tab is the mobile equivalent of focus. */
function onVisibleForSync() {
  if (document.visibilityState === "visible") void syncQuietly();
}

/** Becoming visible is also when the schedule catches up on what it skipped
 *  while the app was hidden. */
function onVisibleForSchedule() {
  if (document.visibilityState === "visible") scheduledCheck();
}

let autoTimer: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  backStack = createBackStack({ onBack: closeTopLayer });
  backStack.sync(openLayers.value.length);
  // Startup sync runs after the first load, so its effects are what the reader
  // sees rather than something the load then overwrites.
  void reload().then(syncQuietly);
  void loadRefreshSettings()
    .then((saved) => {
      if (saved && typeof saved.interval_minutes === "number") {
        refreshSettings.value = saved;
      }
    })
    .catch(() => {
      // The default (off) is the safe reading of an unreadable setting.
    });
  autoTimer = setInterval(scheduledCheck, AUTO_TICK_MS);
  document.addEventListener("keydown", onKeydown);
  window.addEventListener("focus", onWindowFocus);
  document.addEventListener("visibilitychange", onVisibilityChange);
  document.addEventListener("visibilitychange", onVisibleForSchedule);
  document.addEventListener("visibilitychange", onVisibleForSync);
  window.addEventListener("blur", flushOnLeave);
  window.addEventListener("beforeunload", flushOnLeave);
});

onBeforeUnmount(() => {
  backStack?.dispose();
  backStack = null;
  document.removeEventListener("keydown", onKeydown);
  document.removeEventListener("visibilitychange", onVisibilityChange);
  document.removeEventListener("visibilitychange", onVisibleForSchedule);
  document.removeEventListener("visibilitychange", onVisibleForSync);
  if (autoTimer !== null) {
    clearInterval(autoTimer);
    autoTimer = null;
  }
  clearPanelTimers();
  window.removeEventListener("focus", onWindowFocus);
  window.removeEventListener("blur", flushOnLeave);
  window.removeEventListener("beforeunload", flushOnLeave);
  clearPrefixTimer();
  retireUndo();
  void flushBurst();
});

</script>

<template>
  <main class="layout">
    <p v-if="loadError" class="global-error">{{ loadError }}</p>
    <!-- Narrow only: the drawer is an overlay, so the list behind it keeps its
         scroll position and stays inert to taps through the backdrop. -->
    <div
      v-if="narrow && drawerOpen"
      class="drawer-backdrop"
      @click="closeDrawer"
    ></div>
    <Sidebar
      v-show="!narrow || drawerOpen"
      class="sidebar-pane"
      :class="{ drawer: narrow }"
      :feeds="feeds"
      :folders="folders"
      :selection="selection"
      :busy="busy"
      :all-count="counts.all_entries"
      :starred-count="counts.starred_entries"
      :unread-count="counts.unread_entries"
      :narrow="narrow"
      @select="select"
      @changed="reload"
      @remove="doRemove"
      @refresh-one="doRefreshOne"
      @refresh-folder="doRefreshFolder"
      @edit="startEdit"
      @open-settings="openSettings"
    />
    <EntryList
      v-show="!narrow || !readerOpen"
      ref="listRef"
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
      :batch-actions-available="batchActionsAvailable"
      :narrow="narrow"
      :touch="touch"
      @select="selectEntry"
      @toggle-star="toggleStar"
      @refresh-all="doRefreshAll"
      @pull-refresh="refreshCurrentList"
      @set-view="setView"
      @load-more="loadMore"
      @mark-rendered="markRenderedRead"
      @mark-above="markAboveRead"
      @open-nav="openDrawer"
      @context-menu="openRowMenu"
    />
    <ReadingPane
      v-show="!narrow || readerOpen"
      ref="paneRef"
      :entry="selectedEntry"
      :feed="selectedFeed"
      :narrow="narrow"
      :touch="touch"
      :has-previous="hasPreviousEntry"
      :has-next="hasNextEntry"
      @toggle-read="toggleRead"
      @toggle-star="toggleStarOpenEntry"
      @open-link="openLink"
      @close="closeReader"
      @previous="stepEntry(-1)"
      @next="stepEntry(1)"
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
      :last-scheduled="lastScheduledAt"
      @close="settingsOpen = false"
      @changed="reload"
    />
    <KeyboardHelp v-if="helpOpen" @close="helpOpen = false" />
    <TouchMenu
      v-if="menuRow"
      :title="menuRow.entry.title"
      :items="menuItems"
      @select="chooseRowMenu"
      @close="closeRowMenu"
    />
    <!-- One stacked region for both reports, so a batch mark's undo offer and
         a refresh's progress can never cover each other. -->
    <NoticeStack>
      <RefreshPanel
        v-if="panelOpen && panelState"
        :state="panelState"
        :titles="feedTitles"
        @stop="stopRefresh"
        @retry="retryFailedFeeds"
        @dismiss="closePanel"
      />
      <UndoBar
        v-if="pendingUndo"
        :count="pendingUndo.ids.length"
        @undo="undoBatchRead"
      />
    </NoticeStack>
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

/* The minimum hit area for a fingertip. Applied only where the primary pointer
   is a finger, so a pointer layout keeps its compact controls; one definition
   keeps every touch target the same size. */
.touch-target {
  min-width: 44px;
  min-height: 44px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

/* The same minimum, for the controls whose sizing belongs to their component
   rather than to a `touch-target` class. Written once here so a control added
   to a header, a pane, or a dialog cannot quietly end up too small to tap —
   measured on the emulator, the header buttons were 28 CSS px tall. */
:root[data-input="touch"] .list-header button,
:root[data-input="touch"] .pane-header button,
:root[data-input="touch"] .pane-actions .btn,
:root[data-input="touch"] .dialog button,
:root[data-input="touch"] .settings-btn,
:root[data-input="touch"] .row-menu-trigger,
:root[data-input="touch"] .row-menu-item {
  min-width: 44px;
  min-height: 44px;
}
:root[data-input="touch"] .feed-item {
  min-height: 44px;
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
  /* Safe areas: on Android the activity runs edge-to-edge, so the status bar
     and the gesture strip are painted over the webview. The insets are 0 on
     desktop and on devices without cutouts, which is why this applies to every
     layout rather than only the narrow one — a landscape window is wide and
     still has a status bar over its first column. */
  padding-top: env(safe-area-inset-top, 0px);
  padding-bottom: env(safe-area-inset-bottom, 0px);
  padding-left: env(safe-area-inset-left, 0px);
  padding-right: env(safe-area-inset-right, 0px);
  box-sizing: border-box;
}

/* Narrow layout. The layout comes from `useLayout()` so the breakpoint is
   written down once (src/lib/viewport.ts); the CSS only selects on it. */
:root[data-layout="narrow"] .layout {
  grid-template-columns: minmax(0, 1fr);
  /* dvh: the on-screen keyboard and the URL bar shrink the visual viewport on
     Android, and the columns must not extend under them. */
  height: 100dvh;
}

.drawer-backdrop {
  position: absolute;
  inset: 0;
  z-index: 19;
  background: rgba(0, 0, 0, 0.35);
}

/* The sidebar becomes an overlay: out of the grid flow, over the list, with a
   scrim behind it. `v-show` keeps it mounted so its scroll position survives
   opening and closing the drawer.

   The insets are repeated here on purpose: an absolutely positioned box is
   laid out against the ancestor's padding box, so the padding on `.layout`
   does not move it, and without `top` the drawer's first row lands under the
   status bar. */
:root[data-layout="narrow"] .sidebar-pane {
  position: absolute;
  top: env(safe-area-inset-top, 0px);
  bottom: env(safe-area-inset-bottom, 0px);
  left: env(safe-area-inset-left, 0px);
  width: min(20rem, 85vw);
  z-index: 20;
  box-shadow: 0 0 1.5rem rgba(0, 0, 0, 0.25);
  background: #fafafa;
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
