import { mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Sidebar from "../components/Sidebar.vue";
import type { Feed, Folder, Selection } from "../api";
import { FakeBackend, flush } from "./fakeBackend";

/** The IPC bridge the mocked `@tauri-apps/api/core` dispatches into. */
const harness = vi.hoisted(() => ({
  backend: {
    invoke: (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> =>
      Promise.resolve(null),
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) =>
    harness.backend.invoke(cmd, args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: async () => true,
  message: async () => undefined,
}));

let backend: FakeBackend;

/** Seeded so feed 1 holds four unread entries out of six. */
function feed(unread = 4, overrides: Partial<Feed> = {}): Feed {
  return {
    id: 1,
    url: "https://f1.example/feed",
    title: "Feed 1",
    site_url: null,
    last_fetched_at: null,
    last_error: null,
    folder_id: null,
    unread_count: unread,
    ...overrides,
  };
}

const globalSelection: Selection = { kind: "all" };

function render(
  props: Partial<Record<string, unknown>> = {},
): VueWrapper {
  return mount(Sidebar, {
    props: {
      feeds: [feed()],
      folders: [],
      selection: globalSelection,
      busy: false,
      allCount: 6,
      starredCount: 0,
      unreadCount: 4,
      ...props,
    },
  });
}

type MenuTitle = "Feed actions" | "Folder actions" | "All entries actions";

/** Open a row's menu and return its item labels. */
async function menuItems(
  wrapper: VueWrapper,
  title: MenuTitle,
): Promise<string[]> {
  const trigger = wrapper
    .findAll(".row-menu-trigger")
    .find((b) => b.attributes("title") === title);
  if (!trigger) throw new Error(`no ${title} trigger`);
  await trigger.trigger("click");
  const panel = wrapper.find(".row-menu-panel");
  return panel.findAll(".row-menu-item").map((b) => b.text());
}

async function choose(
  wrapper: VueWrapper,
  title: MenuTitle,
  label: string,
): Promise<void> {
  const trigger = wrapper
    .findAll(".row-menu-trigger")
    .find((b) => b.attributes("title") === title);
  if (!trigger) throw new Error(`no ${title} trigger`);
  await trigger.trigger("click");
  const item = wrapper
    .findAll(".row-menu-item")
    .find((b) => b.text() === label);
  if (!item) throw new Error(`no menu item ${label}`);
  await item.trigger("click");
  await flush();
}

beforeEach(() => {
  backend = new FakeBackend({ entries: 6, feeds: 1 });
  harness.backend = backend;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("whole-subscription batch read", () => {
  it("states the feed's unread count on its menu item", async () => {
    const wrapper = render();
    expect(await menuItems(wrapper, "Feed actions")).toContain(
      "Mark all as read (4)",
    );
  });

  it("omits the action when the feed has nothing unread", async () => {
    const wrapper = render({ feeds: [feed(0)] });
    const items = await menuItems(wrapper, "Feed actions");
    expect(items.some((i) => i.startsWith("Mark all as read"))).toBe(false);
  });

  it("states the folder's unread count on its menu item", async () => {
    const folder: Folder = {
      id: 9,
      name: "Tech",
      unread_count: 4,
      feeds: [feed(4, { folder_id: 9 })],
    };
    const wrapper = render({
      feeds: [feed(4, { folder_id: 9 })],
      folders: [folder],
    });
    expect(await menuItems(wrapper, "Folder actions")).toContain(
      "Mark all as read (4)",
    );
  });

  it("marks the whole feed read, including entries never listed", async () => {
    const wrapper = render();
    await choose(wrapper, "Feed actions", "Mark all as read (4)");

    expect(backend.calls[backend.calls.length - 1]).toEqual({
      cmd: "mark_scope_read",
      args: { scope: { kind: "feed", id: 1 } },
    });
    expect(backend.unreadFor(1)).toBe(0);
    expect(wrapper.emitted("changed")).toHaveLength(1);
  });

  it("marks a folder's feeds from the folder row", async () => {
    backend.feeds[0].folder_id = 9;
    const wrapper = render({
      feeds: [feed(4, { folder_id: 9 })],
      folders: [
        { id: 9, name: "Tech", unread_count: 4, feeds: [feed(4, { folder_id: 9 })] },
      ],
    });
    await choose(wrapper, "Folder actions", "Mark all as read (4)");

    expect(backend.calls[backend.calls.length - 1]?.args).toEqual({
      scope: { kind: "folder", id: 9 },
    });
    expect(backend.unreadFor(1)).toBe(0);
  });

  it("surfaces a failed batch mark in the sidebar instead of reloading", async () => {
    harness.backend = {
      invoke: async (cmd: string) => {
        if (cmd === "mark_scope_read") throw new Error("database is locked");
        return null;
      },
    };
    const wrapper = render();
    await choose(wrapper, "Feed actions", "Mark all as read (4)");

    expect(wrapper.find(".error").text()).toContain("database is locked");
    expect(wrapper.emitted("changed")).toBeUndefined();
  });
});

describe("library-wide batch read", () => {
  it("states the library's unread count on the All entries row", async () => {
    const wrapper = render({ unreadCount: 80 });
    expect(await menuItems(wrapper, "All entries actions")).toEqual([
      "Mark all as read (80)",
    ]);
  });

  it("hides the menu when the library has nothing unread", () => {
    const wrapper = render({ unreadCount: 0 });
    const trigger = wrapper
      .findAll(".row-menu-trigger")
      .find((b) => b.attributes("title") === "All entries actions");
    expect(trigger, "nothing for the menu to offer").toBeUndefined();
  });

  it("marks the whole library read regardless of the selection", async () => {
    const wrapper = render({
      unreadCount: 80,
      selection: { kind: "feed", id: 1 },
    });
    await choose(wrapper, "All entries actions", "Mark all as read (80)");

    expect(backend.calls[backend.calls.length - 1]).toEqual({
      cmd: "mark_scope_read",
      args: { scope: { kind: "all" } },
    });
    expect(backend.entries.every((e) => e.is_read)).toBe(true);
    expect(wrapper.emitted("changed")).toHaveLength(1);
  });

  it("surfaces a failure without asking the app to reload", async () => {
    harness.backend = {
      invoke: async (cmd: string) => {
        if (cmd === "mark_scope_read") throw new Error("database is locked");
        return null;
      },
    };
    const wrapper = render({ unreadCount: 80 });
    await choose(wrapper, "All entries actions", "Mark all as read (80)");

    expect(wrapper.find(".error").text()).toContain("database is locked");
    expect(wrapper.emitted("changed")).toBeUndefined();
  });

  it("offers the starred row no batch action", async () => {
    const wrapper = render({ unreadCount: 80 });
    const titles = wrapper
      .findAll(".row-menu-trigger")
      .map((b) => b.attributes("title"));
    expect(titles).toEqual(["All entries actions", "Feed actions"]);
  });
});
