import { invoke } from "@tauri-apps/api/core";

export interface Feed {
  id: number;
  url: string;
  title: string;
  site_url: string | null;
  last_fetched_at: string | null;
  last_error: string | null;
  unread_count: number;
}

export interface Entry {
  id: number;
  feed_id: number;
  title: string;
  link: string | null;
  content_html: string;
  published_at: string | null;
  is_read: boolean;
}

export interface RefreshOutcome {
  feed_id: number;
  error: string | null;
}

export type EntryView = "all" | "unread";

export function listFeeds(): Promise<Feed[]> {
  return invoke<Feed[]>("list_feeds");
}

export function listEntries(view: EntryView, feedId?: number): Promise<Entry[]> {
  return invoke<Entry[]>("list_entries", { view, feedId: feedId ?? null });
}

export function setRead(entryId: number, isRead: boolean): Promise<void> {
  return invoke("set_read", { entryId, isRead });
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
