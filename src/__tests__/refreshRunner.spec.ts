import { describe, expect, it, vi } from "vitest";
import {
  backoffFor,
  createRefreshRunner,
  DEFAULT_CONCURRENCY,
  dueTargets,
  hostOf,
  interleaveByHost,
  isDue,
  isRateLimited,
  jitterFor,
  RATE_LIMIT_DELAYS_MS,
  RATE_LIMIT_STATUSES,
  type RefreshProgress,
  type RefreshSummary,
  type RefreshTarget,
} from "../lib/refresh";
import type { FeedRefresh } from "../api";

function target(feedId: number, url: string, lastFetchedAt: string | null = null): RefreshTarget {
  return { feedId, url, lastFetchedAt };
}

function ok(feedId: number, newEntries = 0): FeedRefresh {
  return { feed_id: feedId, error: null, new_entries: newEntries, http_status: null };
}

function refused(feedId: number, status = 429): FeedRefresh {
  return {
    feed_id: feedId,
    error: `HTTP ${status} Too Many Requests`,
    new_entries: 0,
    http_status: status,
  };
}

function failed(feedId: number, message = "offline"): FeedRefresh {
  return { feed_id: feedId, error: message, new_entries: 0, http_status: null };
}

/** Collects what the runner reports, and lets a test wait for the summary. */
function harness(options: {
  refreshFeed: (feedId: number) => Promise<FeedRefresh>;
  concurrency?: number;
  now?: () => number;
  sleep?: (ms: number) => Promise<void>;
}) {
  const progress: RefreshProgress[] = [];
  let settle!: (summary: RefreshSummary) => void;
  const done = new Promise<RefreshSummary>((resolve) => (settle = resolve));
  const runner = createRefreshRunner({
    refreshFeed: options.refreshFeed,
    concurrency: () => options.concurrency ?? DEFAULT_CONCURRENCY,
    now: options.now,
    sleep: options.sleep ?? (async () => {}),
    onProgress: (p) => progress.push(p),
    onDone: (s) => settle(s),
  });
  return { runner, progress, done };
}

describe("host grouping", () => {
  it("groups by hostname and leaves unparseable urls on their own", () => {
    expect(hostOf("https://Blog.Example.com/feed")).toBe("blog.example.com");
    expect(hostOf("not a url")).toBe("not a url");
  });

  it("interleaves so consecutive dispatches come from different hosts", () => {
    const ordered = interleaveByHost([
      target(1, "https://a.example/1"),
      target(2, "https://a.example/2"),
      target(3, "https://a.example/3"),
      target(4, "https://b.example/1"),
      target(5, "https://c.example/1"),
    ]);

    expect(ordered.map((t) => t.feedId)).toEqual([1, 4, 5, 2, 3]);
  });
});

describe("due eligibility", () => {
  const now = Date.parse("2026-05-01T12:00:00Z");

  it("fetches a feed that was never fetched", () => {
    expect(isDue(target(1, "https://a.example/1"), 30, now)).toBe(true);
  });

  it("skips a feed fetched inside the interval", () => {
    const fetched = new Date(now - 2 * 60_000).toISOString();
    expect(isDue(target(1, "https://a.example/1", fetched), 30, now)).toBe(false);
  });

  it("fetches a feed older than the interval", () => {
    const fetched = new Date(now - 40 * 60_000).toISOString();
    expect(isDue(target(1, "https://a.example/1", fetched), 30, now)).toBe(true);
  });

  it("never schedules a feed before its jittered offset has passed", () => {
    const feedId = 7;
    const jitter = jitterFor(feedId);
    const justInside = new Date(now - (30 * 60_000 + jitter) + 1000).toISOString();
    const justOutside = new Date(now - (30 * 60_000 + jitter) - 1000).toISOString();

    expect(isDue(target(feedId, "https://a.example/7", justInside), 30, now)).toBe(false);
    expect(isDue(target(feedId, "https://a.example/7", justOutside), 30, now)).toBe(true);
  });

  it("keeps only the eligible targets, in their original order", () => {
    const fresh = new Date(now - 60_000).toISOString();
    const old = new Date(now - 24 * 60 * 60_000).toISOString();
    const targets = [
      target(1, "https://a.example/1", old),
      target(2, "https://b.example/1", fresh),
      target(3, "https://c.example/1", null),
    ];

    expect(dueTargets(targets, 30, now).map((t) => t.feedId)).toEqual([1, 3]);
  });
});

describe("a run", () => {
  it("reports every feed, its added entries, and a summary", async () => {
    const { runner, progress, done } = harness({
      refreshFeed: async (feedId) => ok(feedId, feedId === 1 ? 3 : 0),
    });

    runner.start([target(1, "https://a.example/1"), target(2, "https://b.example/1")]);
    const summary = await done;

    expect(summary).toMatchObject({ total: 2, ok: 2, failed: 0, newEntries: 3 });
    expect(summary.stopped).toBe(false);
    expect(progress[progress.length - 1].done).toBe(2);
  });

  it("counts a failed feed without failing the run", async () => {
    const { runner, done } = harness({
      refreshFeed: async (feedId) => (feedId === 2 ? failed(feedId) : ok(feedId)),
    });

    runner.start([target(1, "https://a.example/1"), target(2, "https://b.example/1")]);
    const summary = await done;

    expect(summary).toMatchObject({ total: 2, ok: 1, failed: 1 });
    expect(summary.results.find((r) => r.feedId === 2)?.error).toBe("offline");
  });

  it("treats a command that throws as a failed feed", async () => {
    const { runner, done } = harness({
      refreshFeed: async () => {
        throw new Error("bridge is gone");
      },
    });

    runner.start([target(1, "https://a.example/1")]);
    const summary = await done;

    expect(summary.failed).toBe(1);
    expect(summary.results[0].error).toContain("bridge is gone");
  });

  it("reports an empty target list as an empty run", async () => {
    const { runner, done } = harness({ refreshFeed: async (feedId) => ok(feedId) });
    runner.start([]);
    await expect(done).resolves.toMatchObject({ total: 0, ok: 0, failed: 0 });
  });
});

describe("concurrency", () => {
  it("never has two requests in flight to the same host", async () => {
    let inFlight = new Map<string, number>();
    let overlapped = false;
    const { runner, done } = harness({
      refreshFeed: async (feedId) => {
        const host = `h${feedId % 2}`;
        const now = (inFlight.get(host) ?? 0) + 1;
        inFlight.set(host, now);
        if (now > 1) overlapped = true;
        await new Promise((resolve) => setTimeout(resolve, 1));
        inFlight.set(host, (inFlight.get(host) ?? 1) - 1);
        return ok(feedId);
      },
    });

    runner.start([
      target(1, "https://one.example/1"),
      target(2, "https://one.example/2"),
      target(3, "https://one.example/3"),
      target(4, "https://one.example/4"),
    ]);
    await done;

    expect(overlapped).toBe(false);
  });

  it("starts feeds on other hosts while one host works through its queue", async () => {
    const started: number[] = [];
    const { runner, done } = harness({
      concurrency: 2,
      refreshFeed: async (feedId) => {
        started.push(feedId);
        await new Promise((resolve) => setTimeout(resolve, 1));
        return ok(feedId);
      },
    });

    runner.start([
      target(1, "https://one.example/1"),
      target(2, "https://one.example/2"),
      target(3, "https://two.example/1"),
    ]);
    await done;

    // Interleaved, so the second host starts before the first host's queue is
    // drained: `started` reads 1, 3, 2 rather than 1, 2, 3.
    expect(started).toEqual([1, 3, 2]);
  });

  it("respects the overall limit", async () => {
    let inFlight = 0;
    let peak = 0;
    const { runner, done } = harness({
      concurrency: 2,
      refreshFeed: async (feedId) => {
        inFlight += 1;
        peak = Math.max(peak, inFlight);
        await new Promise((resolve) => setTimeout(resolve, 1));
        inFlight -= 1;
        return ok(feedId);
      },
    });

    runner.start(
      Array.from({ length: 6 }, (_, i) =>
        target(i + 1, `https://h${i + 1}.example/feed`),
      ),
    );
    await done;

    expect(peak).toBe(2);
  });

  it("leaves the configured gap between two requests to one host", async () => {
    const slept: number[] = [];
    let clock = 0;
    const { runner, done } = harness({
      now: () => clock,
      sleep: async (ms) => {
        slept.push(ms);
        clock += ms;
      },
      refreshFeed: async (feedId) => {
        clock += 5;
        return ok(feedId);
      },
    });

    runner.start([
      target(1, "https://one.example/1"),
      target(2, "https://one.example/2"),
      target(3, "https://one.example/3"),
    ]);
    await done;

    expect(slept).toEqual([300, 300]);
  });
});

describe("rate limiting", () => {
  it("holds a host's remaining feeds instead of failing them", async () => {
    let clock = 0;
    let refusedOnce = false;
    const finishedOrder: number[] = [];
    const { runner, done } = harness({
      now: () => clock,
      sleep: async (ms) => {
        clock += ms;
      },
      refreshFeed: async (feedId) => {
        if (feedId !== 2 && !refusedOnce) {
          refusedOnce = true;
          return refused(feedId);
        }
        finishedOrder.push(feedId);
        return ok(feedId);
      },
    });

    runner.start([
      target(1, "https://one.example/1"),
      target(2, "https://two.example/1"),
      target(3, "https://one.example/2"),
    ]);
    const summary = await done;

    // The other host finished while the refused host was waiting, and the
    // refused feeds came back as successes rather than failures.
    expect(finishedOrder).toEqual([2, 1, 3]);
    expect(summary).toMatchObject({ total: 3, ok: 3, failed: 0 });
  });

  it("publishes the backoff while the host waits", async () => {
    let clock = 0;
    let refusals = 0;
    const progress: RefreshProgress[] = [];
    let settle!: (summary: RefreshSummary) => void;
    const done = new Promise<RefreshSummary>((resolve) => (settle = resolve));
    const runner = createRefreshRunner({
      refreshFeed: async (feedId) => {
        // One refusal for this host, so the run then finishes and the test can
        // read the backoff it published while waiting.
        if (refusals === 0) {
          refusals += 1;
          return refused(feedId);
        }
        return ok(feedId);
      },
      now: () => clock,
      sleep: async (ms) => {
        clock += ms;
      },
      onProgress: (p) => progress.push(p),
      onDone: (s) => settle(s),
    });

    runner.start([target(1, "https://one.example/1"), target(2, "https://one.example/2")]);
    await done;

    const reported = progress.find((p) => p.backoffs.length > 0);
    expect(reported).toBeDefined();
    expect(reported?.backoffs[0]).toMatchObject({ host: "one.example", waiting: 2 });
  });

  it("retries the deferred feeds after the delay, and a success clears the error", async () => {
    const answers = [refused(1), ok(1)];
    let clock = 0;
    const calls: number[] = [];
    const { runner, done } = harness({
      now: () => clock,
      sleep: async (ms) => {
        clock += ms;
      },
      refreshFeed: async (feedId) => {
        calls.push(clock);
        return answers.shift() ?? ok(feedId);
      },
    });

    runner.start([target(1, "https://one.example/1")]);
    const summary = await done;

    expect(calls).toHaveLength(2);
    expect(calls[1] - calls[0]).toBeGreaterThanOrEqual(RATE_LIMIT_DELAYS_MS[0]);
    expect(summary).toMatchObject({ ok: 1, failed: 0 });
  });

  it("waits longer after a second refusal", async () => {
    const waits: number[] = [];
    let clock = 0;
    let refusals = 0;
    let settle!: (summary: RefreshSummary) => void;
    const done = new Promise<RefreshSummary>((resolve) => (settle = resolve));
    const runner = createRefreshRunner({
      refreshFeed: async (feedId) => {
        refusals += 1;
        return refusals <= 2 ? refused(feedId) : ok(feedId);
      },
      now: () => clock,
      sleep: async (ms) => {
        waits.push(ms);
        clock += ms;
      },
      onDone: (s) => settle(s),
    });

    runner.start([target(1, "https://one.example/1")]);
    await done;

    // The wait runs a millisecond past the deadline, so the retry is not a
    // hair too early for the clock that recorded the refusal.
    const backoffWaits = waits.filter((ms) => ms >= RATE_LIMIT_DELAYS_MS[0]);
    expect(backoffWaits).toEqual([
      RATE_LIMIT_DELAYS_MS[0] + 1,
      RATE_LIMIT_DELAYS_MS[1] + 1,
    ]);
  });

  it("only treats refusals as rate limits", () => {
    for (const status of RATE_LIMIT_STATUSES) expect(isRateLimited(status)).toBe(true);
    expect(isRateLimited(404)).toBe(false);
    expect(isRateLimited(null)).toBe(false);
    expect(backoffFor(1)).toBe(RATE_LIMIT_DELAYS_MS[0]);
    expect(backoffFor(9)).toBe(RATE_LIMIT_DELAYS_MS[RATE_LIMIT_DELAYS_MS.length - 1]);
  });
});

describe("single flight", () => {
  it("does not dispatch a feed twice when triggered again", async () => {
    const calls: number[] = [];
    const { runner, done } = harness({
      refreshFeed: async (feedId) => {
        calls.push(feedId);
        await new Promise((resolve) => setTimeout(resolve, 1));
        return ok(feedId);
      },
    });

    const targets = [target(1, "https://a.example/1"), target(2, "https://b.example/1")];
    runner.start(targets);
    runner.start(targets);
    await done;

    expect(calls.sort()).toEqual([1, 2]);
  });

  it("adds feeds a narrow trigger asked for while the run is in flight", async () => {
    const seen: number[] = [];
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const { runner, done } = harness({
      refreshFeed: async (feedId) => {
        seen.push(feedId);
        if (feedId === 1) await gate;
        return ok(feedId);
      },
    });

    runner.start([target(1, "https://a.example/1")]);
    await new Promise((resolve) => setTimeout(resolve, 0));
    runner.start([
      target(1, "https://a.example/1"),
      target(2, "https://b.example/1"),
      target(3, "https://c.example/1"),
    ]);
    release();
    const summary = await done;

    expect(seen.sort()).toEqual([1, 2, 3]);
    expect(summary.total).toBe(3);
  });

  it("reports itself as running until the summary lands", async () => {
    const { runner, done } = harness({
      refreshFeed: async (feedId) => {
        await new Promise((resolve) => setTimeout(resolve, 1));
        return ok(feedId);
      },
    });

    expect(runner.isRunning()).toBe(false);
    runner.start([target(1, "https://a.example/1")]);
    expect(runner.isRunning()).toBe(true);
    await done;
    expect(runner.isRunning()).toBe(false);
  });
});

describe("stopping", () => {
  it("fetches nothing more once stopped, and keeps what finished", async () => {
    const calls: number[] = [];
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    let settle!: (summary: RefreshSummary) => void;
    const done = new Promise<RefreshSummary>((resolve) => (settle = resolve));
    const runner = createRefreshRunner({
      concurrency: () => 1,
      refreshFeed: async (feedId) => {
        calls.push(feedId);
        await gate;
        return ok(feedId, 2);
      },
      onDone: (s) => settle(s),
    });

    runner.start([
      target(1, "https://a.example/1"),
      target(2, "https://b.example/1"),
      target(3, "https://c.example/1"),
    ]);
    await new Promise((resolve) => setTimeout(resolve, 0));
    runner.cancel();
    release();
    const summary = await done;

    expect(calls).toEqual([1]);
    expect(summary.stopped).toBe(true);
    expect(summary.newEntries).toBe(calls.length * 2);
  });

  it("keeps the result of a feed that was in flight when the reader stopped", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const { runner, done } = harness({
      refreshFeed: async (feedId) => {
        await gate;
        return ok(feedId, 4);
      },
    });

    runner.start([target(1, "https://a.example/1")]);
    await new Promise((resolve) => setTimeout(resolve, 0));
    runner.cancel();
    release();
    const summary = await done;

    expect(summary.stopped).toBe(true);
    expect(summary.results).toHaveLength(1);
    expect(summary.newEntries).toBe(4);
  });

  it("reports the stop as soon as it is asked for", async () => {
    const progress: RefreshProgress[] = [];
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const runner = createRefreshRunner({
      refreshFeed: async (feedId) => {
        await gate;
        return ok(feedId);
      },
      onProgress: (p) => progress.push(p),
    });

    runner.start([target(1, "https://a.example/1")]);
    await new Promise((resolve) => setTimeout(resolve, 0));
    runner.cancel();

    // The feed is still in flight, but the panel must already say so.
    expect(progress[progress.length - 1].stopping).toBe(true);
    release();
    await new Promise((resolve) => setTimeout(resolve, 5));
  });

  it("does not call back after a run has completed", async () => {
    const onDone = vi.fn();
    const runner = createRefreshRunner({
      refreshFeed: async (feedId) => ok(feedId),
      onDone,
    });

    runner.start([target(1, "https://a.example/1")]);
    await new Promise((resolve) => setTimeout(resolve, 5));
    runner.cancel();
    await new Promise((resolve) => setTimeout(resolve, 5));

    expect(onDone).toHaveBeenCalledTimes(1);
  });
});

describe("a host that keeps refusing", () => {
  it("gives up after the configured retries and reports the feeds as failed", async () => {
    let clock = 0;
    let attempts = 0;
    const { runner, done } = harness({
      now: () => clock,
      sleep: async (ms) => {
        clock += ms;
      },
      refreshFeed: async (feedId) => {
        attempts += 1;
        return refused(feedId);
      },
    });

    runner.start([target(1, "https://one.example/1")]);
    const summary = await done;

    expect(attempts).toBe(RATE_LIMIT_DELAYS_MS.length + 1);
    expect(summary).toMatchObject({ total: 1, ok: 0, failed: 1 });
    expect(summary.results[0].status).toBe(429);
  });
});
