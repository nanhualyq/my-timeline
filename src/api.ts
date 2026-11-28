import { invoke } from "@tauri-apps/api/core";

export interface Feed {
  id: number;
  url: string;
  title: string;
  site_url: string | null;
  last_fetched_at: string | null;
  last_error: string | null;
  /** null means uncategorized */
  folder_id: number | null;
  unread_count: number;
}

export interface Folder {
  id: number;
  name: string;
  unread_count: number;
  feeds: Feed[];
}

export interface Entry {
  id: number;
  feed_id: number;
  title: string;
  link: string | null;
  content_html: string;
  published_at: string | null;
  is_read: boolean;
  is_starred: boolean;
}

/**
 * One row of the paginated entry list. Deliberately carries no
 * `content_html`: the list renders a cover, a title, a short excerpt, and a
 * date, and bodies are fetched on demand for the entry being read.
 */
export interface EntryListItem {
  id: number;
  feed_id: number;
  title: string;
  link: string | null;
  /** Plain-text snippet derived at ingest; empty when the entry has no text. */
  excerpt: string;
  /** Absolute cover image URL, when the feed declared or contained one. */
  image_url: string | null;
  published_at: string | null;
  /** Ordering position — publication time when known, arrival time otherwise.
   * Echoed back as the cursor for the next page. */
  sort_key: string;
  is_read: boolean;
  is_starred: boolean;
}

/**
 * Keyset cursor: the ordering position and id of the last row of the previous
 * page. Preferred over an offset so a refresh cannot shift the page the reader
 * is on.
 */
export interface Cursor {
  sort: string;
  id: number;
}

export interface RefreshOutcome {
  feed_id: number;
  error: string | null;
}

export type EntryView = "all" | "unread";

/** Aggregate counts for the two global sidebar rows. Both count read and
 * unread entries alike. */
export interface EntryCounts {
  all_entries: number;
  starred_entries: number;
  /** Unread entries across the whole library, for the global batch action. */
  unread_entries: number;
}

/** Which entries to list. The starred view is global: it never combines
 * with a feed or folder selection. */
export type EntryScope =
  | { kind: "all" }
  | { kind: "feed"; id: number }
  | { kind: "folder"; id: number }
  | { kind: "starred" };

/** Sidebar selection: global views, a folder, or a single feed. */
export type Selection =
  | { kind: "all" }
  | { kind: "starred" }
  | { kind: "folder"; id: number }
  | { kind: "feed"; id: number };

export function listFeeds(): Promise<Feed[]> {
  return invoke<Feed[]>("list_feeds");
}

export function listFolders(): Promise<Folder[]> {
  return invoke<Folder[]>("list_folders");
}

export function listEntries(
  view: EntryView,
  scope: EntryScope,
  after: Cursor | null,
  limit: number,
): Promise<EntryListItem[]> {
  return invoke<EntryListItem[]>("list_entries", { view, scope, after, limit });
}

/** How many entries match a view and scope, for the list's progress total. */
export function countEntries(
  view: EntryView,
  scope: EntryScope,
): Promise<number> {
  return invoke<number>("count_entries", { view, scope });
}

/** One entry with its content, for the reading pane. Null when it is gone. */
export function getEntry(entryId: number): Promise<Entry | null> {
  return invoke<Entry | null>("get_entry", { entryId });
}

export function setRead(entryId: number, isRead: boolean): Promise<void> {
  return invoke("set_read", { entryId, isRead });
}

/** Mark the given entries read in one round trip. The ids come from rows the
 * list has already shown, so this never reaches an entry the reader has not
 * seen. Resolves with the number of rows whose state actually changed. */
export function markReadMany(ids: number[]): Promise<number> {
  return invoke<number>("mark_read_many", { ids });
}

/** Mark every unread entry of a scope read, including entries that were never
 * listed. Resolves with the number of rows whose state actually changed. */
export function markScopeRead(scope: EntryScope): Promise<number> {
  return invoke<number>("mark_scope_read", { scope });
}

export function entryCounts(): Promise<EntryCounts> {
  return invoke<EntryCounts>("entry_counts");
}

export function setStarred(entryId: number, isStarred: boolean): Promise<void> {
  return invoke("set_starred", { entryId, isStarred });
}

export function removeFeed(feedId: number): Promise<void> {
  return invoke("remove_feed", { feedId });
}

export function addFeed(url: string): Promise<Feed> {
  return invoke<Feed>("add_feed", { url });
}

export function refreshFeed(feedId: number): Promise<void> {
  return invoke("refresh_feed", { feedId });
}

export function refreshAll(): Promise<RefreshOutcome[]> {
  return invoke<RefreshOutcome[]>("refresh_all");
}

/** Refresh every feed under a folder; resolves with per-feed outcomes. */
export function refreshFolder(folderId: number): Promise<RefreshOutcome[]> {
  return invoke<RefreshOutcome[]>("refresh_folder", { folderId });
}

export interface ImportOutcome {
  feeds_added: number;
  feeds_skipped: number;
  folders_created: number;
}
export interface ExportOutcome {
  feeds_exported: number;
  /** The rendered document; the frontend writes it where the picker pointed. */
  xml: string;
}

export function createFolder(name: string): Promise<Folder> {
  return invoke<Folder>("create_folder", { name });
}

export function renameFolder(folderId: number, name: string): Promise<void> {
  return invoke("rename_folder", { folderId, name });
}

export function deleteFolder(folderId: number): Promise<void> {
  return invoke("delete_folder", { folderId });
}

export function moveFeedToFolder(
  feedId: number,
  folderId: number | null,
): Promise<void> {
  return invoke("move_feed_to_folder", { feedId, folderId });
}

/** Set a feed's custom display title, or clear it with `null` to revert
 * to the title provided by the feed. */
export function setFeedTitle(
  feedId: number,
  title: string | null,
): Promise<void> {
  return invoke("set_feed_title", { feedId, title });
}

/** Import OPML text. Merge-only; does not fetch any feed. */
export function importOpml(xml: string): Promise<ImportOutcome> {
  return invoke("import_opml", { xml });
}

/** Render every subscription as OPML text and report how many were written. */
export function exportOpml(): Promise<ExportOutcome> {
  return invoke("export_opml");
}

// --- Sync document -------------------------------------------------------
//
// The portable form of the state more than one installation shares. Entry
// bodies are absent by design: every installation re-fetches those. Records are
// keyed by a copyable identity (feed URL, folder uuid) and carry the counter of
// the edit that set each field, so two copies resolve a conflict without
// consulting a clock.

export interface SyncFolderRecord {
  uuid: string;
  name: string;
  name_ctr: number;
  sync_ctr: number;
}

export interface SyncFeedRecord {
  url: string;
  /** The reader's own title; a feed-provided title is re-derived per device. */
  custom_title?: string | null;
  title_ctr: number;
  /** The uuid of the folder the feed is filed under. */
  folder?: string | null;
  folder_ctr: number;
  sync_ctr: number;
}

export interface SyncMarkRecord {
  feed: string;
  guid: string;
  /** The entry's ordering day, which bounds how long a read mark is shared. */
  day: string;
  /** Absent means never set, which is not the same as an explicit false. */
  read?: boolean | null;
  read_ctr: number;
  star?: boolean | null;
  star_ctr: number;
}

export interface SyncTombstoneRecord {
  /** "feed" or "folder". */
  kind: string;
  /** The feed URL, or the folder uuid. */
  key: string;
  ctr: number;
  day: string;
}

export interface SyncDocument {
  doc_version: number;
  /** The day before which read marks are not shared. */
  read_horizon_day: string;
  folders: SyncFolderRecord[];
  feeds: SyncFeedRecord[];
  marks: SyncMarkRecord[];
  tombstones: SyncTombstoneRecord[];
  /** Reserved for reader settings; this build neither writes nor reads it. */
  settings?: unknown;
}

export interface SyncExportOutcome {
  /** The rendered document; the frontend writes it where the picker pointed. */
  text: string;
  feeds_exported: number;
  folders_exported: number;
  marks_exported: number;
}

export interface SyncImportSummary {
  folders_added: number;
  folders_changed: number;
  folders_removed: number;
  feeds_added: number;
  feeds_changed: number;
  feeds_removed: number;
  stars_applied: number;
  read_marks_applied: number;
  /** Marks kept for entries this installation has not fetched yet. */
  marks_recorded_for_later: number;
}

/** Render the shared library state as sync-document text. */
export function exportSyncDocument(): Promise<SyncExportOutcome> {
  return invoke("export_sync_document");
}

/** Merge sync-document text. Merge-only; does not fetch any feed. */
export function importSyncDocument(text: string): Promise<SyncImportSummary> {
  return invoke("import_sync_document", { text });
}

// --- WebDAV sync ---------------------------------------------------------

/** Where the sync document lives, plus the credential decision for it. The
 *  password is stored in the same local database as everything else. */
export interface SyncConfig {
  url: string;
  path: string;
  username: string;
  password: string;
  accept_invalid_certs: boolean;
}

export type SyncOutcome = "synced" | "created" | "nothing_changed" | "failed";

export interface SyncStatus {
  /** RFC3339, so the dialog can say when rather than only what. */
  ran_at: string;
  outcome: SyncOutcome;
  /** What was applied, or why the cycle failed — already a sentence. */
  detail: string;
}

export interface SyncConnectionTest {
  document_exists: boolean;
  document_url: string;
}

/** The saved connection, or null when the form has never been filled in. */
export function loadSyncConfig(): Promise<SyncConfig | null> {
  return invoke<SyncConfig | null>("load_sync_config");
}

export function saveSyncConfig(config: SyncConfig): Promise<void> {
  return invoke("save_sync_config", { config });
}

/** Check the saved connection without syncing or changing anything. */
export function testSyncConnection(): Promise<SyncConnectionTest> {
  return invoke<SyncConnectionTest>("test_sync_connection");
}

/** Run one sync cycle. `force` bypasses the focus throttle, never the
 *  "one sync at a time" rule. Rejects only when nothing was attempted. */
export function syncNow(force: boolean): Promise<SyncStatus> {
  return invoke<SyncStatus>("sync_now", { force });
}

/** The last sync's outcome, for a dialog opened after a background sync. */
export function syncStatus(): Promise<SyncStatus | null> {
  return invoke<SyncStatus | null>("sync_status");
}
