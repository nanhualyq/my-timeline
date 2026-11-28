import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";

/**
 * The platform-specific half of sync-document interchange, shaped exactly like
 * `opml-file.ts`: a picked file is not always a filesystem path, and the two
 * platforms converge on the same two steps — ask the picker, then read or write
 * through the reference it returned. The Rust side only ever sees text.
 */

const SYNC_FILTER = { name: "Sync document", extensions: ["json"] };

const DEFAULT_FILE_NAME = "my-timeline-sync.json";

/**
 * Android filters by MIME type and ignores extensions, so the extension form
 * matches nothing there and hides real files. Accepting the generic JSON, text,
 * and binary types is what makes a document that arrived by download or by
 * `adb push` selectable at all.
 */
function syncFilter(): { name: string; extensions: string[] } {
  const android =
    typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
  return android
    ? {
        name: "Sync document",
        extensions: ["application/json", "text/plain", "application/octet-stream"],
      }
    : SYNC_FILTER;
}

/** The picked document plus its text, or null when the reader cancelled. */
export async function pickSyncText(): Promise<string | null> {
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [syncFilter()],
    // `copy` is the default and the safe choice on mobile: the file is handed
    // to the app instead of leaving a scoped handle open on someone's storage.
    fileAccessMode: "copy",
  });
  if (picked === null) return null;
  const reference = Array.isArray(picked) ? picked[0] : picked;
  if (!reference) return null;
  return readTextFile(reference);
}

/** Ask where to write an export. Returns null when the reader cancelled. */
export async function pickSyncDestination(): Promise<string | null> {
  return save({
    defaultPath: DEFAULT_FILE_NAME,
    filters: [syncFilter()],
  });
}

/** Write rendered document text to the reference the save dialog returned. */
export async function writeSyncText(
  reference: string,
  text: string,
): Promise<void> {
  await writeTextFile(reference, text);
}
