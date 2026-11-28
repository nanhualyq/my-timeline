import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flush } from "./fakeBackend";

/** The platform-facing plugins are mocked; everything above them is real. */
const harness = vi.hoisted(() => ({
  invoke: vi.fn(),
  open: vi.fn(),
  save: vi.fn(),
  readTextFile: vi.fn(),
  writeTextFile: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) =>
    harness.invoke(cmd, args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => harness.open(...args),
  save: (...args: unknown[]) => harness.save(...args),
  message: async () => undefined,
  confirm: async () => true,
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readTextFile: (...args: unknown[]) => harness.readTextFile(...args),
  writeTextFile: (...args: unknown[]) => harness.writeTextFile(...args),
}));

import {
  pickSyncDestination,
  pickSyncText,
  writeSyncText,
} from "../lib/sync-file";
import SettingsDialog from "../components/SettingsDialog.vue";

const DOCUMENT = JSON.stringify({
  doc_version: 1,
  read_horizon_day: "2026-01-01",
  folders: [],
  feeds: [],
  marks: [],
  tombstones: [],
});

const EMPTY_SUMMARY = {
  folders_added: 0,
  folders_changed: 0,
  folders_removed: 0,
  feeds_added: 0,
  feeds_changed: 0,
  feeds_removed: 0,
  stars_applied: 0,
  read_marks_applied: 0,
  marks_recorded_for_later: 0,
};

beforeEach(() => {
  vi.clearAllMocks();
  harness.invoke.mockResolvedValue(null);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("sync document file access", () => {
  it("reads the text of the file the picker returned", async () => {
    harness.open.mockResolvedValue("/home/me/my-timeline-sync.json");
    harness.readTextFile.mockResolvedValue(DOCUMENT);

    await expect(pickSyncText()).resolves.toBe(DOCUMENT);
    expect(harness.readTextFile).toHaveBeenCalledWith(
      "/home/me/my-timeline-sync.json",
    );
  });

  it("asks the picker to hand the file over rather than leave it in place", async () => {
    harness.open.mockResolvedValue("/home/me/my-timeline-sync.json");
    harness.readTextFile.mockResolvedValue(DOCUMENT);

    await pickSyncText();

    const options = harness.open.mock.calls[0][0] as Record<string, unknown>;
    expect(options.fileAccessMode).toBe("copy");
    expect(options.directory).toBe(false);
    expect(options.multiple).toBe(false);
  });

  it("does not read anything when the picker is cancelled", async () => {
    harness.open.mockResolvedValue(null);

    await expect(pickSyncText()).resolves.toBeNull();
    expect(harness.readTextFile).not.toHaveBeenCalled();
  });

  it("writes the document to the reference the save dialog returned", async () => {
    harness.save.mockResolvedValue("/home/me/out.json");
    harness.writeTextFile.mockResolvedValue(undefined);

    const destination = await pickSyncDestination();
    expect(destination).toBe("/home/me/out.json");

    await writeSyncText(destination!, DOCUMENT);
    expect(harness.writeTextFile).toHaveBeenCalledWith(
      "/home/me/out.json",
      DOCUMENT,
    );
  });

  it("reports no destination when the save dialog is cancelled", async () => {
    harness.save.mockResolvedValue(null);
    await expect(pickSyncDestination()).resolves.toBeNull();
  });
});

describe("sync picker filters", () => {
  async function filterFor(userAgent: string) {
    vi.stubGlobal("navigator", { userAgent });
    harness.open.mockResolvedValue(null);
    await pickSyncText();
    const calls = harness.open.mock.calls;
    const options = calls[calls.length - 1][0] as {
      filters: { name: string; extensions: string[] }[];
    };
    return options.filters[0].extensions;
  }

  it("filters by extension on desktop", async () => {
    await expect(
      filterFor("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"),
    ).resolves.toEqual(["json"]);
  });

  it("filters by MIME type on Android", async () => {
    // Android ignores extensions and reads these strings as MIME types, so a
    // filter of json matches nothing and hides real documents.
    const extensions = await filterFor("Mozilla/5.0 (Linux; Android 17; Pixel)");
    expect(extensions).toContain("application/json");
    expect(extensions).toContain("text/plain");
    expect(extensions).toContain("application/octet-stream");
    expect(extensions).not.toContain("json");
  });
});

describe("settings dialog sync actions", () => {
  function mountDialog() {
    return mount(SettingsDialog, {
      props: { busy: false, lastScheduled: null },
      global: { stubs: { teleport: true } },
    });
  }

  it("imports the chosen document and reports what it applied", async () => {
    harness.open.mockResolvedValue("/tmp/sync.json");
    harness.readTextFile.mockResolvedValue(DOCUMENT);
    harness.invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "import_sync_document") {
        return {
          ...EMPTY_SUMMARY,
          feeds_added: 2,
          folders_added: 1,
          stars_applied: 3,
        };
      }
      return null;
    });

    const wrapper = mountDialog();
    await wrapper.get(".row.sync button").trigger("click");
    await flush();

    expect(harness.invoke).toHaveBeenCalledWith("import_sync_document", {
      text: DOCUMENT,
    });
    const notice = wrapper.get(".dialog-notice").text();
    expect(notice).toContain("2 feed(s) added");
    expect(notice).toContain("1 folder(s) added");
    expect(notice).toContain("3 star(s)");
    expect(wrapper.emitted("changed")).toBeTruthy();
  });

  it("imports nothing when the picker is cancelled", async () => {
    harness.open.mockResolvedValue(null);
    const wrapper = mountDialog();

    await wrapper.get(".row.sync button").trigger("click");
    await flush();

    // The dialog reads the saved WebDAV connection when it opens; what must not
    // happen is an import.
    expect(harness.invoke).not.toHaveBeenCalledWith(
      "import_sync_document",
      expect.anything(),
    );
    expect(wrapper.find(".dialog-notice").exists()).toBe(false);
    expect(wrapper.find(".dialog-error").exists()).toBe(false);
  });

  it("reports a rejected document as an error and leaves the message area alone", async () => {
    harness.open.mockResolvedValue("/tmp/future.json");
    harness.readTextFile.mockResolvedValue('{"doc_version": 99}');
    harness.invoke.mockRejectedValue(
      "unsupported sync document version 99; this build supports 1",
    );

    const wrapper = mountDialog();
    await wrapper.get(".row.sync button").trigger("click");
    await flush();

    expect(wrapper.get(".dialog-error").text()).toContain("version 99");
    expect(wrapper.find(".dialog-notice").exists()).toBe(false);
    expect(wrapper.emitted("changed")).toBeFalsy();
  });

  it("exports the rendered document to the chosen destination", async () => {
    harness.save.mockResolvedValue("/tmp/out.json");
    harness.writeTextFile.mockResolvedValue(undefined);
    harness.invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "export_sync_document") {
        return {
          text: DOCUMENT,
          feeds_exported: 3,
          folders_exported: 1,
          marks_exported: 7,
        };
      }
      return null;
    });

    const wrapper = mountDialog();
    const buttons = wrapper.findAll(".row.sync button");
    await buttons[1].trigger("click");
    await flush();

    expect(harness.invoke).toHaveBeenCalledWith("export_sync_document", undefined);
    expect(harness.writeTextFile).toHaveBeenCalledWith("/tmp/out.json", DOCUMENT);
    expect(wrapper.get(".dialog-notice").text()).toContain("3 feeds");
  });

  it("writes nothing when the save dialog is cancelled", async () => {
    harness.save.mockResolvedValue(null);
    const wrapper = mountDialog();

    const buttons = wrapper.findAll(".row.sync button");
    await buttons[1].trigger("click");
    await flush();

    expect(harness.invoke).not.toHaveBeenCalledWith(
      "export_sync_document",
      expect.anything(),
    );
    expect(harness.writeTextFile).not.toHaveBeenCalled();
  });
});
