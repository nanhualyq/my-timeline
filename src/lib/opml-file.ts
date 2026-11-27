import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";

/**
 * The platform-specific half of OPML interchange.
 *
 * A picked file is not always a filesystem path: on Android the document
 * picker hands back a reference into another app's storage. Both platforms
 * converge on the same two steps — ask the picker, then read or write through
 * the reference it returned — so the reader never has to copy a file by hand,
 * and the Rust side only ever sees OPML text.
 */

const OPML_FILTER = { name: "OPML", extensions: ["opml", "xml"] };

/**
 * Android filters by MIME type and ignores extensions; desktop filters by
 * extension. Measured on a device: a real `.opml` file reached the picker as
 * `application/octet-stream` (shown as a "BIN file") and every tap on it did
 * nothing, because `["opml","xml"]` is read there as two MIME types that
 * match no document. Accepting the generic XML and binary types is what makes
 * an OPML file that arrived by download or by `adb push` selectable at all.
 */
function opmlFilter(): { name: string; extensions: string[] } {
  const android =
    typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
  return android
    ? {
        name: "OPML",
        extensions: ["text/xml", "application/xml", "application/octet-stream"],
      }
    : OPML_FILTER;
}

/** The picked document plus its text, or null when the reader cancelled. */
export async function pickOpmlText(): Promise<string | null> {
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [opmlFilter()],
    // `copy` is the default and the safe choice on mobile: the file is handed
    // to the app instead of leaving a scoped handle open on someone's storage.
    fileAccessMode: "copy",
  });
  if (picked === null) return null;
  // `multiple: false` always yields one path; the array form is for callers
  // that ask for several.
  const reference = Array.isArray(picked) ? picked[0] : picked;
  if (!reference) return null;
  return readTextFile(reference);
}

/** Ask where to write an export. Returns null when the reader cancelled. */
export async function pickOpmlDestination(): Promise<string | null> {
  return save({
    defaultPath: "subscriptions.opml",
    filters: [opmlFilter()],
  });
}

/** Write rendered OPML to the reference the save dialog returned. */
export async function writeOpmlText(
  reference: string,
  xml: string,
): Promise<void> {
  await writeTextFile(reference, xml);
}
