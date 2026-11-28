import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flush } from "./fakeBackend";

/** The two platform-facing plugins are mocked; everything above them is real. */
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
  pickOpmlDestination,
  pickOpmlText,
  writeOpmlText,
} from "../lib/opml-file";
import SettingsDialog from "../components/SettingsDialog.vue";

const OPML = `<?xml version="1.0"?><opml version="2.0"><body>
  <outline text="Tech"><outline type="rss" text="A" xmlUrl="https://a.example/feed"/></outline>
</body></opml>`;

beforeEach(() => {
  vi.clearAllMocks();
  harness.invoke.mockResolvedValue(null);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("OPML file access", () => {
  it("reads the text of the file the picker returned", async () => {
    harness.open.mockResolvedValue("/home/me/subscriptions.opml");
    harness.readTextFile.mockResolvedValue(OPML);

    await expect(pickOpmlText()).resolves.toBe(OPML);
    expect(harness.readTextFile).toHaveBeenCalledWith("/home/me/subscriptions.opml");
  });

  it("asks the picker to hand the file over rather than leave it in place", async () => {
    harness.open.mockResolvedValue("/home/me/subscriptions.opml");
    harness.readTextFile.mockResolvedValue(OPML);

    await pickOpmlText();

    const options = harness.open.mock.calls[0][0] as Record<string, unknown>;
    // `copy` is what makes an Android document reference readable at all.
    expect(options.fileAccessMode).toBe("copy");
    expect(options.directory).toBe(false);
    expect(options.multiple).toBe(false);
  });

  it("does not read anything when the picker is cancelled", async () => {
    harness.open.mockResolvedValue(null);

    await expect(pickOpmlText()).resolves.toBeNull();
    expect(harness.readTextFile).not.toHaveBeenCalled();
  });

  it("surfaces a picker reference that cannot be read", async () => {
    harness.open.mockResolvedValue("content://com.android.providers/document/1234");
    harness.readTextFile.mockRejectedValue(new Error("forbidden path"));

    await expect(pickOpmlText()).rejects.toThrow("forbidden path");
  });

  it("writes the document to the reference the save dialog returned", async () => {
    harness.save.mockResolvedValue("/home/me/export.opml");
    harness.writeTextFile.mockResolvedValue(undefined);

    const destination = await pickOpmlDestination();
    expect(destination).toBe("/home/me/export.opml");

    await writeOpmlText(destination!, OPML);
    expect(harness.writeTextFile).toHaveBeenCalledWith(
      "/home/me/export.opml",
      OPML,
    );
  });

  it("reports no destination when the save dialog is cancelled", async () => {
    harness.save.mockResolvedValue(null);
    await expect(pickOpmlDestination()).resolves.toBeNull();
  });
});

describe("settings dialog OPML actions", () => {
  function mountDialog() {
    // The dialog teleports to <body>; the stub keeps it inside the wrapper so
    // the assertions can address it directly.
    return mount(SettingsDialog, {
      props: { busy: false },
      global: { stubs: { teleport: true } },
    });
  }

  it("imports the chosen file's text and reports the outcome", async () => {
    harness.open.mockResolvedValue("/tmp/subs.opml");
    harness.readTextFile.mockResolvedValue(OPML);
    harness.invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "import_opml") {
        return { feeds_added: 2, feeds_skipped: 1, folders_created: 1 };
      }
      return null;
    });

    const wrapper = mountDialog();
    await wrapper.get(".row.opml button").trigger("click");
    await flush();

    expect(harness.invoke).toHaveBeenCalledWith("import_opml", { xml: OPML });
    expect(wrapper.get(".dialog-notice").text()).toContain("2 added");
    expect(wrapper.emitted("changed")).toBeTruthy();
  });

  it("imports nothing when the picker is cancelled", async () => {
    harness.open.mockResolvedValue(null);
    const wrapper = mountDialog();

    await wrapper.get(".row.opml button").trigger("click");
    await flush();

    // The dialog reads the saved WebDAV connection when it opens; what must not
    // happen is an import.
    expect(harness.invoke).not.toHaveBeenCalledWith("import_opml", expect.anything());
    expect(wrapper.find(".dialog-notice").exists()).toBe(false);
  });

  it("exports the rendered document to the chosen destination", async () => {
    harness.save.mockResolvedValue("/tmp/out.opml");
    harness.invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "export_opml") {
        return { feeds_exported: 3, xml: OPML };
      }
      return null;
    });

    const wrapper = mountDialog();
    const buttons = wrapper.findAll(".row.opml button");
    await buttons[1].trigger("click");
    await flush();

    expect(harness.invoke).toHaveBeenCalledWith("export_opml", undefined);
    expect(harness.writeTextFile).toHaveBeenCalledWith("/tmp/out.opml", OPML);
    expect(wrapper.get(".dialog-notice").text()).toContain("3 feeds");
  });

  it("writes nothing when the save dialog is cancelled", async () => {
    harness.save.mockResolvedValue(null);
    const wrapper = mountDialog();

    const buttons = wrapper.findAll(".row.opml button");
    await buttons[1].trigger("click");
    await flush();

    expect(harness.invoke).not.toHaveBeenCalledWith("export_opml", expect.anything());
    expect(harness.writeTextFile).not.toHaveBeenCalled();
  });

  it("reports an unreadable file as an error, not as a silent no-op", async () => {
    harness.open.mockResolvedValue("/tmp/locked.opml");
    harness.readTextFile.mockRejectedValue(new Error("cannot read /tmp/locked.opml"));

    const wrapper = mountDialog();
    await wrapper.get(".row.opml button").trigger("click");
    await flush();

    expect(wrapper.get(".dialog-error").text()).toContain("cannot read");
    expect(harness.invoke).not.toHaveBeenCalledWith("import_opml", expect.anything());
  });
});

describe("OPML picker filters", () => {
  async function filterFor(userAgent: string) {
    vi.stubGlobal("navigator", { userAgent });
    harness.open.mockResolvedValue(null);
    await pickOpmlText();
    const calls = harness.open.mock.calls;
    const options = calls[calls.length - 1][0] as {
      filters: { name: string; extensions: string[] }[];
    };
    return options.filters[0].extensions;
  }

  it("filters by extension on desktop", async () => {
    await expect(filterFor("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).resolves.toEqual([
      "opml",
      "xml",
    ]);
  });

  it("filters by MIME type on Android", async () => {
    // Android ignores extensions and treats these strings as MIME types, so a
    // filter of opml/xml matches nothing and hides real files.
    const extensions = await filterFor("Mozilla/5.0 (Linux; Android 17; Pixel)");
    expect(extensions).toContain("text/xml");
    expect(extensions).toContain("application/xml");
    expect(extensions).toContain("application/octet-stream");
    expect(extensions).not.toContain("opml");
  });
});
