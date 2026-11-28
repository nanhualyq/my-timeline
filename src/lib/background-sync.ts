import { loadSyncConfig, syncNow } from "../api";
import type { SyncStatus } from "../api";

/**
 * The background half of WebDAV sync.
 *
 * A trigger that fires on startup or when the window regains focus must never
 * interrupt reading: an unconfigured connection and a failed sync are both
 * reported in Settings, where the reader went to configure it in the first
 * place. The throttle lives in the backend, so calling this often is free.
 */

/** Whether a status means the library changed and the view must be refreshed. */
export function appliedChanges(status: SyncStatus | null): boolean {
  return status?.outcome === "synced" || status?.outcome === "created";
}

/**
 * Sync if a connection is configured, and call `onApplied` when the sync
 * changed anything. Returns the status, or null when nothing ran or it failed.
 */
export async function runBackgroundSync(
  onApplied: () => void | Promise<void>,
): Promise<SyncStatus | null> {
  const config = await loadSyncConfig().catch(() => null);
  // No connection: no sync command is sent at all.
  if (!config || !config.url.trim()) return null;

  let status: SyncStatus;
  try {
    status = await syncNow(false);
  } catch {
    // Nothing was attempted, or the cycle could not start; Settings reports it.
    return null;
  }
  if (appliedChanges(status)) await onApplied();
  return status;
}
