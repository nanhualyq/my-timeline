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
  applied = false,
): SyncStatus {
  return { ran_at: "2026-11-28T10:00:00+00:00", outcome, detail, applied };
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
    props: { busy: false, lastScheduled: null },
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

  it("asks for a refresh when the sync only downloaded changes", async () => {
    backendWith({
      save_sync_config: () => null,
      sync_now: () => status("nothing_changed", "1 feed(s) added", true),
    });

    const wrapper = mountDialog();
    await flush();
    await fillConnection(wrapper);
    await wrapper.findAll(".sync-actions button")[2].trigger("click");
    await flush();

    expect(wrapper.get(".dialog-status").text()).toContain("1 feed(s) added");
    expect(wrapper.emitted("changed")).toBeTruthy();
  });

  it("leaves the view alone when the sync applied nothing", async () => {
    backendWith({
      save_sync_config: () => null,
      sync_now: () => status("nothing_changed", "no changes on either side"),
    });

    const wrapper = mountDialog();
    await flush();
    await fillConnection(wrapper);
    await wrapper.findAll(".sync-actions button")[2].trigger("click");
    await flush();

    expect(wrapper.emitted("changed")).toBeFalsy();
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

describe("the refresh schedule", () => {
  it("shows the saved schedule and persists a change", async () => {
    const saved: { enabled: boolean; interval_minutes: number }[] = [];
    backendWith({
      load_refresh_settings: () => ({ enabled: true, interval_minutes: 60 }),
      save_refresh_settings: () => null,
    });
    const wrapper = mountDialog();
    await flush();

    const checkbox = wrapper.get(".schedule-switch input");
    expect((checkbox.element as HTMLInputElement).checked).toBe(true);
    expect((wrapper.get(".schedule-interval").element as HTMLSelectElement).value).toBe("60");

    await wrapper.get(".schedule-interval").setValue("15");
    await flush();

    const calls = harness.invoke.mock.calls.filter((c) => c[0] === "save_refresh_settings");
    expect(calls).toHaveLength(1);
    expect(calls[0][1]).toEqual({ settings: { enabled: true, interval_minutes: 15 } });
    saved.push(calls[0][1].settings);
    expect(wrapper.text()).toContain("Refreshing every 15 minutes");
  });

  it("turns the schedule off and says so", async () => {
    backendWith({
      load_refresh_settings: () => ({ enabled: true, interval_minutes: 30 }),
      save_refresh_settings: () => null,
    });
    const wrapper = mountDialog();
    await flush();

    await wrapper.get(".schedule-switch input").setValue(false);
    await flush();

    const calls = harness.invoke.mock.calls.filter((c) => c[0] === "save_refresh_settings");
    expect(calls[0][1]).toEqual({ settings: { enabled: false, interval_minutes: 30 } });
    expect(wrapper.text()).toContain("Scheduled refresh is off");
  });

  it("reports when the last scheduled refresh ran and when the next is due", async () => {
    backendWith({
      load_refresh_settings: () => ({ enabled: true, interval_minutes: 30 }),
    });
    const wrapper = mount(SettingsDialog, {
      props: { busy: false, lastScheduled: "2026-11-28T10:00:00+00:00" },
      global: { stubs: { teleport: true } },
    });
    await flush();

    const text = wrapper.text();
    expect(text).toContain("Last scheduled refresh");
    expect(text).toContain("Next check");
  });

  it("offers a fixed set of intervals rather than a free number", async () => {
    backendWith({ load_refresh_settings: () => ({ enabled: false, interval_minutes: 30 }) });
    const wrapper = mountDialog();
    await flush();

    const options = wrapper.findAll(".schedule-interval option");
    expect(options.map((o) => o.attributes("value"))).toEqual(["15", "30", "60", "180"]);
    expect((wrapper.get(".schedule-interval").element as HTMLSelectElement).disabled).toBe(true);
  });
});
