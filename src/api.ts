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
  position: number;
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

export interface RefreshOutcome {
  feed_id: number;
  error: string | null;
}

export type EntryView = "all" | "unread";

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

export function listEntries(view: EntryView, scope: EntryScope): Promise<Entry[]> {
  return invoke<Entry[]>("list_entries", { view, scope });
}

export function setRead(entryId: number, isRead: boolean): Promise<void> {
  return invoke("set_read", { entryId, isRead });
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

export interface ImportOutcome {
  feeds_added: number;
  feeds_skipped: number;
  folders_created: number;
}

export interface ExportOutcome {
  feeds_exported: number;
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

export function reorderFolders(orderedIds: number[]): Promise<void> {
  return invoke("reorder_folders", { orderedIds });
}

export function moveFeedToFolder(
  feedId: number,
  folderId: number | null,
): Promise<void> {
  return invoke("move_feed_to_folder", { feedId, folderId });
}

/** Import an OPML file. Merge-only; does not fetch any feed. */
export function importOpml(path: string): Promise<ImportOutcome> {
  return invoke("import_opml", { path });
}

export function exportOpml(path: string): Promise<ExportOutcome> {
  return invoke("export_opml", { path });
}
