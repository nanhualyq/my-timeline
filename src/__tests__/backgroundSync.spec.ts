import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SyncConfig, SyncStatus } from "../api";

const harness = vi.hoisted(() => ({
  load: vi.fn(),
  syncNow: vi.fn(),
}));

vi.mock("../api", () => ({
  loadSyncConfig: () => harness.load(),
  syncNow: (force: boolean) => harness.syncNow(force),
}));

import { appliedChanges, runBackgroundSync } from "../lib/background-sync";

const CONFIG: SyncConfig = {
  url: "https://dav.example/dav",
  path: "",
  username: "me",
  password: "secret",
  accept_invalid_certs: false,
};

function status(outcome: SyncStatus["outcome"], detail = "detail", applied = false): SyncStatus {
  return { ran_at: new Date(0).toISOString(), outcome, detail, applied };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("what a status means for the view", () => {
  it("a sync that changed the library needs a refresh", () => {
    expect(appliedChanges(status("synced", "detail", true))).toBe(true);
    expect(appliedChanges(status("created", "detail", true))).toBe(true);
  });

  it("a download-only sync changed the library even though the document did not", () => {
    expect(appliedChanges(status("nothing_changed", "1 feed(s) added", true))).toBe(true);
  });

  it("a status from a build without the field falls back to the outcome", () => {
    expect(appliedChanges(status("synced"))).toBe(true);
    expect(appliedChanges(status("created"))).toBe(true);
  });

  it("a sync with nothing to do, a failure, and no sync at all do not", () => {
    expect(appliedChanges(status("nothing_changed"))).toBe(false);
    expect(appliedChanges(status("failed"))).toBe(false);
    expect(appliedChanges(status("failed", "lost the race", true))).toBe(true);
    expect(appliedChanges(null)).toBe(false);
  });
});

describe("background sync", () => {
  it("sends no sync command when no connection is configured", async () => {
    harness.load.mockResolvedValue(null);
    const reload = vi.fn();

    const result = await runBackgroundSync(reload);

    expect(result).toBeNull();
    expect(harness.syncNow).not.toHaveBeenCalled();
    expect(reload).not.toHaveBeenCalled();
  });

  it("treats a saved form with an empty URL as unconfigured", async () => {
    harness.load.mockResolvedValue({ ...CONFIG, url: "   " });
    const reload = vi.fn();

    await runBackgroundSync(reload);

    expect(harness.syncNow).not.toHaveBeenCalled();
  });

  it("refreshes the view only when the sync applied something", async () => {
    harness.load.mockResolvedValue(CONFIG);

    harness.syncNow.mockResolvedValue(status("synced", "detail", true));
    const reload = vi.fn();
    await runBackgroundSync(reload);
    expect(harness.syncNow).toHaveBeenCalledWith(false);
    expect(reload).toHaveBeenCalledTimes(1);

    reload.mockClear();
    harness.syncNow.mockResolvedValue(status("nothing_changed", "1 feed(s) added", true));
    await runBackgroundSync(reload);
    expect(reload).toHaveBeenCalledTimes(1);

    reload.mockClear();
    harness.syncNow.mockResolvedValue(status("nothing_changed"));
    await runBackgroundSync(reload);
    expect(reload).not.toHaveBeenCalled();
  });

  it("stays quiet when the sync cannot start", async () => {
    harness.load.mockResolvedValue(CONFIG);
    harness.syncNow.mockRejectedValue("a sync is already running");
    const reload = vi.fn();

    await expect(runBackgroundSync(reload)).resolves.toBeNull();
    expect(reload).not.toHaveBeenCalled();
  });

  it("stays quiet when reading the saved connection fails", async () => {
    harness.load.mockRejectedValue(new Error("database is locked"));
    const reload = vi.fn();

    await expect(runBackgroundSync(reload)).resolves.toBeNull();
    expect(harness.syncNow).not.toHaveBeenCalled();
  });
});
