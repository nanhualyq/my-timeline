import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flush } from "./fakeBackend";
import type { SyncConfig, SyncStatus } from "../api";

/** Only the IPC bridge is mocked; the dialog's own logic is the subject. */
const harness = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) =>
    harness.invoke(cmd, args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  message: async () => undefined,
  confirm: async () => true,
  open: async () => null,
  save: async () => null,
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readTextFile: async () => "",
  writeTextFile: async () => undefined,
}));

import SettingsDialog from "../components/SettingsDialog.vue";

const SAVED: SyncConfig = {
  url: "https://dav.example/dav",
  path: "state.json",
  username: "me",
  password: "secret",
  accept_invalid_certs: true,
};

function status(
  outcome: SyncStatus["outcome"],
  detail: string,
): SyncStatus {
  return { ran_at: "2026-11-28T10:00:00+00:00", outcome, detail };
}

type Handlers = Record<string, () => unknown>;

/** Answers the dialog's commands; anything unlisted answers null. */
function backendWith(handlers: Handlers) {
  harness.invoke.mockImplementation(async (cmd: string) =>
    cmd in handlers ? handlers[cmd]() : null,
  );
}

function mountDialog() {
  return mount(SettingsDialog, {
    props: { busy: false },
    global: { stubs: { teleport: true } },
  });
}

/** Fill the connection form the way a reader would. */
async function fillConnection(
  wrapper: ReturnType<typeof mountDialog>,
  url = "https://dav.example/dav",
) {
  const inputs = wrapper.findAll(".sync-connection input");
  await inputs[0].setValue(url);
  await inputs[1].setValue("state.json");
  await inputs[2].setValue("me");
  await inputs[3].setValue("secret");
}

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("WebDAV section", () => {
  it("shows the saved connection and the last sync", async () => {
    backendWith({
      load_sync_config: () => SAVED,
      sync_status: () =>
        status("nothing_changed", "no changes on either side"),
    });

    const wrapper = mountDialog();
    await flush();

    const inputs = wrapper.findAll(".sync-connection input");
    expect((inputs[0].element as HTMLInputElement).value).toBe(SAVED.url);
    expect((inputs[1].element as HTMLInputElement).value).toBe(SAVED.path);
    expect((inputs[2].element as HTMLInputElement).value).toBe(SAVED.username);
    expect((inputs[3].element as HTMLInputElement).value).toBe(SAVED.password);
    expect(
      (wrapper.get(".checkbox input").element as HTMLInputElement).checked,
    ).toBe(true);
    expect(wrapper.get(".dialog-status").text()).toContain("Last sync");
    expect(wrapper.get(".dialog-status").text()).toContain(
      "no changes on either side",
    );
  });

  it("says so when nothing is configured yet", async () => {
    backendWith({ load_sync_config: () => null, sync_status: () => null });

    const wrapper = mountDialog();
    await flush();

    expect(wrapper.find(".dialog-status").exists()).toBe(false);
    const save = wrapper.get(".sync-actions button");
    expect(save.attributes("disabled")).toBeDefined();
  });

  it("saves the form and keeps the values on screen", async () => {
    backendWith({ save_sync_config: () => null });

    const wrapper = mountDialog();
    await flush();
    await fillConnection(wrapper);
    await wrapper.get(".sync-actions form, form.sync-connection").trigger("submit");
    await flush();

    expect(harness.invoke).toHaveBeenCalledWith("save_sync_config", {
      config: {
        url: "https://dav.example/dav",
        path: "state.json",
        username: "me",
        password: "secret",
        accept_invalid_certs: false,
      },
    });
    expect(wrapper.get(".dialog-notice").text()).toContain("saved");
    const url = wrapper.findAll(".sync-connection input")[0];
    expect((url.element as HTMLInputElement).value).toBe("https://dav.example/dav");
  });

  it("reports a rejected credential from the connection test", async () => {
    backendWith({
      save_sync_config: () => null,
      test_sync_connection: () => {
        throw "the server rejected the username or password (HTTP 401)";
      },
    });

    const wrapper = mountDialog();
    await flush();
    await fillConnection(wrapper);
    const buttons = wrapper.findAll(".sync-actions button");
    await buttons[1].trigger("click");
    await flush();

    expect(wrapper.get(".dialog-error").text()).toContain("username or password");
    expect(wrapper.find(".dialog-notice").exists()).toBe(false);
  });

  it("reports a working connection and whether the document exists", async () => {
    backendWith({
      save_sync_config: () => null,
      test_sync_connection: () => ({
        document_exists: false,
        document_url: "https://dav.example/dav/state.json",
      }),
    });

    const wrapper = mountDialog();
    await flush();
    await fillConnection(wrapper);
    await wrapper.findAll(".sync-actions button")[1].trigger("click");
    await flush();

    expect(wrapper.get(".dialog-notice").text()).toContain(
      "will be created on the first sync",
    );
  });

  it("shows the outcome of Sync now without closing the dialog", async () => {
    backendWith({
      save_sync_config: () => null,
      sync_now: () => status("synced", "2 feed(s) added, 3 star(s)"),
    });

    const wrapper = mountDialog();
    await flush();
    await fillConnection(wrapper);
    await wrapper.findAll(".sync-actions button")[2].trigger("click");
    await flush();

    expect(harness.invoke).toHaveBeenCalledWith("sync_now", { force: true });
    expect(wrapper.get(".dialog-status").text()).toContain("2 feed(s) added");
    expect(wrapper.emitted("changed")).toBeTruthy();
    expect(wrapper.emitted("close")).toBeFalsy();
  });

  it("keeps the reason a sync failed", async () => {
    backendWith({
      save_sync_config: () => null,
      sync_now: () => status("failed", "cannot reach the server: connection refused"),
    });

    const wrapper = mountDialog();
    await flush();
    await fillConnection(wrapper);
    await wrapper.findAll(".sync-actions button")[2].trigger("click");
    await flush();

    const line = wrapper.get(".dialog-status").text();
    expect(line).toContain("failed");
    expect(line).toContain("cannot reach the server");
    expect(wrapper.emitted("changed")).toBeFalsy();
  });

  it("does not sync when no server URL is filled in", async () => {
    backendWith({ save_sync_config: () => null });

    const wrapper = mountDialog();
    await flush();
    for (const button of wrapper.findAll(".sync-actions button")) {
      expect(button.attributes("disabled")).toBeDefined();
    }
    expect(harness.invoke).not.toHaveBeenCalledWith("sync_now", expect.anything());
  });
});
