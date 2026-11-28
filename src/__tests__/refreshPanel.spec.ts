import { enableAutoUnmount, mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import { nextTick } from "vue";
import RefreshPanel from "../components/RefreshPanel.vue";
import NoticeStack from "../components/NoticeStack.vue";
import UndoBar from "../components/UndoBar.vue";
import type { FeedResult, RefreshProgress, RefreshSummary } from "../lib/refresh";

enableAutoUnmount(afterEach);

/** Feed names by id, as the sidebar knows them. */
const TITLES: Record<number, string> = {
  1: "Ars Technica",
  2: "Hacker News",
  3: "arXiv",
  4: "The Verge",
};

function result(feedId: number, overrides: Partial<FeedResult> = {}): FeedResult {
  return {
    feedId,
    ok: true,
    newEntries: 0,
    error: null,
    status: null,
    ...overrides,
  };
}

function progress(overrides: Partial<RefreshProgress> = {}): RefreshProgress {
  return { total: 24, done: 7, results: [], backoffs: [], stopping: false, ...overrides };
}

function summary(overrides: Partial<RefreshSummary> = {}): RefreshSummary {
  return {
    total: 24,
    ok: 24,
    failed: 0,
    newEntries: 12,
    stopped: false,
    results: [],
    ...overrides,
  };
}

describe("the refresh panel while a run is in flight", () => {
  it("shows how many feeds are done and how many there are", () => {
    const wrapper = mount(RefreshPanel, {
      props: { state: { phase: "running", progress: progress() }, titles: TITLES },
    });

    expect(wrapper.text()).toContain("Refreshing");
    expect(wrapper.text()).toContain("7 / 24");
  });

  it("lists the finished feeds with what they added", () => {
    const wrapper = mount(RefreshPanel, {
      props: {
        state: {
          phase: "running",
          progress: progress({
            results: [result(1, { newEntries: 3 }), result(2)],
          }),
        },
        titles: TITLES,
      },
    });

    expect(wrapper.findAll(".panel-row")).toHaveLength(2);
    expect(wrapper.text()).toContain("+3");
  });

  it("names a failing feed and why it failed", () => {
    const wrapper = mount(RefreshPanel, {
      props: {
        state: {
          phase: "running",
          progress: progress({
            results: [result(4, { ok: false, error: "HTTP 500 Internal Server Error" })],
          }),
        },
        titles: TITLES,
      },
    });

    const failed = wrapper.get(".panel-row.failed");
    expect(failed.text()).toContain("The Verge");
    expect(failed.text()).toContain("HTTP 500");
  });

  it("says which host is refusing requests", () => {
    const wrapper = mount(RefreshPanel, {
      props: {
        state: {
          phase: "running",
          progress: progress({
            backoffs: [{ host: "one.example", waiting: 3, until: Date.now() + 60_000 }],
          }),
        },
        titles: TITLES,
      },
    });

    expect(wrapper.text()).toContain("one.example is refusing requests");
    expect(wrapper.text()).toContain("3 feeds waiting");
  });

  it("says a stop is under way and stops offering it", () => {
    const wrapper = mount(RefreshPanel, {
      props: {
        state: { phase: "running", progress: progress({ stopping: true }) },
        titles: TITLES,
      },
    });

    expect(wrapper.text()).toContain("Stopping");
    expect(wrapper.find(".panel-action").exists()).toBe(false);
  });

  it("offers a stop action", async () => {
    const wrapper = mount(RefreshPanel, {
      props: { state: { phase: "running", progress: progress() }, titles: TITLES },
    });

    await wrapper.get(".panel-action").trigger("click");

    expect(wrapper.emitted("stop")).toHaveLength(1);
  });
});

describe("the refresh panel when a run ends", () => {
  it("summarizes feeds, added entries, and failures", () => {
    const wrapper = mount(RefreshPanel, {
      props: {
        state: {
          phase: "done",
          summary: summary({ ok: 22, failed: 2 }),
        },
        titles: TITLES,
      },
    });

    const text = wrapper.text();
    expect(text).toContain("24 feeds");
    expect(text).toContain("12 new");
    expect(text).toContain("2 failed");
  });

  it("says a run was stopped", () => {
    const wrapper = mount(RefreshPanel, {
      props: {
        state: { phase: "done", summary: summary({ stopped: true, ok: 3 }) },
        titles: TITLES,
      },
    });

    expect(wrapper.text()).toContain("stopped");
  });

  it("offers a retry, and only for the feeds that failed", async () => {
    const wrapper = mount(RefreshPanel, {
      props: {
        state: {
          phase: "done",
          summary: summary({
            failed: 1,
            results: [result(1), result(2, { ok: false, error: "HTTP 503" })],
          }),
        },
        titles: TITLES,
      },
    });

    expect(wrapper.findAll(".panel-row.failed")).toHaveLength(1);
    const retry = wrapper.findAll(".panel-action").find((b) => b.text().includes("Retry"));
    expect(retry).toBeDefined();
    await retry!.trigger("click");

    expect(wrapper.emitted("retry")).toHaveLength(1);
  });

  it("offers no retry when nothing failed", () => {
    const wrapper = mount(RefreshPanel, {
      props: { state: { phase: "done", summary: summary() }, titles: TITLES },
    });

    expect(wrapper.find(".panel-foot button").exists()).toBe(false);
  });

  it("can be dismissed", async () => {
    const wrapper = mount(RefreshPanel, {
      props: { state: { phase: "done", summary: summary() }, titles: TITLES },
    });

    await wrapper.get("[aria-label='Dismiss refresh report']").trigger("click");

    expect(wrapper.emitted("dismiss")).toHaveLength(1);
  });

  it("announces the summary politely rather than the live counter", () => {
    const wrapper = mount(RefreshPanel, {
      props: { state: { phase: "done", summary: summary() }, titles: TITLES },
    });

    const status = wrapper.get("[role='status']");
    expect(status.attributes("aria-live")).toBe("polite");
  });
});

describe("the notice stack", () => {
  it("stacks an undo offer and a refresh report instead of overlapping them", async () => {
    const wrapper = mount({
      components: { NoticeStack, UndoBar, RefreshPanel },
      template: `
        <NoticeStack>
          <RefreshPanel :state="{ phase: 'running', progress }" :titles="{}" />
          <UndoBar :count="2" />
        </NoticeStack>
      `,
      data() {
        return { progress: progress() };
      },
    });
    await nextTick();

    const stack = wrapper.get(".notice-stack");
    expect(stack.findAll(".refresh-panel")).toHaveLength(1);
    expect(stack.findAll(".undo-bar")).toHaveLength(1);
    // One column, so the two can be measured against each other rather than
    // both pinning themselves to the same corner.
    expect(stack.classes()).toContain("notice-stack");
  });

  it("does not let the region swallow taps meant for the list", () => {
    const wrapper = mount(NoticeStack, { slots: { default: "<p>hi</p>" } });
    expect(wrapper.get(".notice-stack").attributes("style")).toBeUndefined();
  });
});
