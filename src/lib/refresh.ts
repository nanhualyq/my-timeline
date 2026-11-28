import type { FeedRefresh } from "../api";

/**
 * The refresh runner.
 *
 * Every trigger — the list's action, its keystroke, the pull gesture, the
 * schedule, and returning to the foreground — starts or extends a run here,
 * so there is exactly one queue, one concurrency policy, and one progress
 * story. The backend fetches one feed per call and knows nothing about
 * ordering, so all of that is decided in this file.
 *
 * Two constraints apply to every dispatch:
 *
 * - A bounded number of requests in flight overall.
 * - Never more than one request in flight to the same host, plus a gap between
 *   two requests to it. Feeds are grouped by hostname for that, and the queue
 *   is interleaved across groups: dispatching one host's feeds back to back
 *   would leave the global limit with nothing to do but wait on that host.
 *
 * A run is also the unit of de-duplication: a feed already dispatched is never
 * dispatched again, so mashing the trigger, pulling during a scheduled run, or
 * coming back to the app mid-run cannot fetch anything twice.
 */

/** How many feeds one run fetches at a time, on a pointer layout. */
export const DEFAULT_CONCURRENCY = 5;

/** A cellular connection gains nothing from five sockets and loses latency. */
export const TOUCH_CONCURRENCY = 3;

/** Spacing between two requests to the same host, in milliseconds. */
export const HOST_GAP_MS = 300;

/**
 * How long a rate-limited host is left alone, by how many times it has refused
 * during this run. The last value is the ceiling, and its length is also the
 * number of retries a host gets: a host that keeps refusing must not be able
 * to hold a run open forever.
 */
export const RATE_LIMIT_DELAYS_MS = [60_000, 300_000];

/** Statuses that mean "not now", as opposed to "this feed is broken". */
export const RATE_LIMIT_STATUSES: readonly number[] = [429, 503];

/** Up to a minute of per-feed spread on top of the interval. */
export const DUE_JITTER_MS = 60_000;

/** A feed the run could consider fetching. */
export interface RefreshTarget {
  feedId: number;
  url: string;
  /** RFC3339 of the last fetch, or null when the feed was never fetched. */
  lastFetchedAt: string | null;
}

/** One finished feed. */
export interface FeedResult {
  feedId: number;
  ok: boolean;
  newEntries: number;
  error: string | null;
  status: number | null;
}

/** A host the run is leaving alone for a while. */
export interface HostBackoff {
  host: string;
  /** How many feeds are waiting behind this delay. */
  waiting: number;
  /** When the delay ends, in epoch milliseconds. */
  until: number;
}

export interface RefreshProgress {
  /** Feeds this run covers, retries included once. */
  total: number;
  /** Feeds finished so far. */
  done: number;
  results: FeedResult[];
  backoffs: HostBackoff[];
  /** The reader stopped the run: nothing more is dispatched, and whatever is
   *  already in flight is still on its way back. */
  stopping: boolean;
}

export interface RefreshSummary {
  total: number;
  ok: number;
  failed: number;
  newEntries: number;
  stopped: boolean;
  results: FeedResult[];
}

/** What the panel is showing: a run in flight, or the last run's summary. */
export type RefreshPanelState =
  | { phase: "running"; progress: RefreshProgress }
  | { phase: "done"; summary: RefreshSummary };

export interface RefreshDeps {
  /** One fetch. Injected so the runner is testable without IPC. */
  refreshFeed: (feedId: number) => Promise<FeedRefresh>;
  /** A run started with this many feeds. */
  onStart?: (total: number) => void;
  onProgress?: (progress: RefreshProgress) => void;
  onDone?: (summary: RefreshSummary) => void;
  /** Requests in flight at once. Read per run, so it follows the viewport. */
  concurrency?: () => number;
  now?: () => number;
  sleep?: (ms: number) => Promise<void>;
}

export interface RefreshRunner {
  /** True between the first dispatch and the run's completion. */
  isRunning(): boolean;
  /**
   * Start a run, or extend the one in flight with the targets it does not
   * cover yet. Never starts a second concurrent run.
   */
  start(targets: RefreshTarget[]): void;
  /** Stop dispatching. Feeds already in flight finish and keep their results. */
  cancel(): void;
}

/** A target's host group. Unparseable URLs group under their own text, so two
 *  broken feeds never wait on each other. */
export function hostOf(url: string): string {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch {
    return url;
  }
}

/**
 * The per-feed offset on top of the interval. Derived from the id so a batch of
 * feeds added from one import does not share a due time for the rest of its
 * life.
 */
export function jitterFor(feedId: number): number {
  const widened = Math.abs(Math.trunc(feedId)) * 2654435761;
  return widened % DUE_JITTER_MS;
}

/** Whether a feed's content is older than the interval. */
export function isDue(
  target: RefreshTarget,
  intervalMinutes: number,
  now: number,
): boolean {
  const stamp = target.lastFetchedAt === null ? NaN : Date.parse(target.lastFetchedAt);
  if (!Number.isFinite(stamp)) return true;
  const age = intervalMinutes * 60_000 + jitterFor(target.feedId);
  return now - stamp >= age;
}

/** The targets a scheduled or resume check may fetch, in their original order. */
export function dueTargets(
  targets: RefreshTarget[],
  intervalMinutes: number,
  now: number,
): RefreshTarget[] {
  return targets.filter((target) => isDue(target, intervalMinutes, now));
}

/** Whether a feed's result means its host should be left alone for a while. */
export function isRateLimited(status: number | null): boolean {
  return status !== null && RATE_LIMIT_STATUSES.includes(status);
}

/** The delay after `refusals` refusals from one host. */
export function backoffFor(refusals: number): number {
  const index = Math.min(Math.max(refusals, 1), RATE_LIMIT_DELAYS_MS.length) - 1;
  return RATE_LIMIT_DELAYS_MS[index];
}

/**
 * Round-robin the host groups into one queue, so consecutive dispatches come
 * from different hosts wherever the run has more than one.
 */
export function interleaveByHost(
  targets: RefreshTarget[],
): RefreshTarget[] {
  const groups = new Map<string, RefreshTarget[]>();
  for (const target of targets) {
    const host = hostOf(target.url);
    const group = groups.get(host);
    if (group) group.push(target);
    else groups.set(host, [target]);
  }
  const queues = [...groups.values()];
  const ordered: RefreshTarget[] = [];
  let drained = false;
  while (!drained) {
    drained = true;
    for (const queue of queues) {
      const next = queue.shift();
      if (next === undefined) continue;
      ordered.push(next);
      drained = false;
    }
  }
  return ordered;
}

/** The run's mutable state. One per start-to-finish run. */
class Run {
  /** Every feed this run has taken responsibility for: no second dispatch. */
  private readonly covered = new Set<number>();
  /** Remaining work per host, in the order the feeds were queued. */
  private readonly queues = new Map<string, RefreshTarget[]>();
  private readonly busyHosts = new Set<string>();
  private readonly backoffs = new Map<string, { refusals: number; until: number }>();
  /** When this host's last request finished, for the inter-request gap. */
  private readonly lastRequestAt = new Map<string, number>();
  private readonly results: FeedResult[] = [];
  private stopped = false;
  private finished = false;
  private readonly concurrency: number;
  private readonly now: () => number;
  private readonly sleep: (ms: number) => Promise<void>;

  constructor(private readonly deps: RefreshDeps) {
    this.concurrency = Math.max(1, deps.concurrency?.() ?? DEFAULT_CONCURRENCY);
    this.now = deps.now ?? (() => Date.now());
    this.sleep = deps.sleep ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
  }

  get isRunning(): boolean {
    return !this.finished;
  }

  /** Add the targets this run does not cover yet. Returns how many were added. */
  add(targets: RefreshTarget[]): number {
    const fresh = targets.filter((target) => !this.covered.has(target.feedId));
    if (fresh.length === 0) {
      // A trigger with nothing new still settles the run, so an empty library
      // reports an empty summary instead of hanging.
      this.pump();
      return 0;
    }
    for (const target of fresh) {
      this.covered.add(target.feedId);
      const host = hostOf(target.url);
      const queue = this.queues.get(host);
      if (queue) queue.push(target);
      else this.queues.set(host, [target]);
    }
    this.pump();
    return fresh.length;
  }

  cancel(): void {
    this.stopped = true;
    // Whatever has not been dispatched yet is abandoned; the feeds already in
    // flight still finish and keep their results.
    for (const host of [...this.queues.keys()]) this.queues.delete(host);
    // Say so at once: an in-flight request cannot be recalled, and a stop that
    // only takes effect a timeout later must not look like a dead button.
    this.report();
    this.pump();
  }

  /**
   * Start every host chain the limits allow, then report or finish. One chain
   * holds one request at a time, so the busy hosts are the in-flight requests.
   */
  private pump() {
    if (this.finished) return;
    if (!this.stopped) {
      for (const host of [...this.queues.keys()]) {
        if (this.busyHosts.size >= this.concurrency) break;
        if (this.busyHosts.has(host)) continue;
        if ((this.queues.get(host)?.length ?? 0) === 0) {
          this.queues.delete(host);
          continue;
        }
        void this.drainHost(host);
      }
    }
    if (this.busyHosts.size === 0 && this.queues.size === 0) this.finish();
  }

  /** One host's queue, one request at a time, in order. */
  private async drainHost(host: string) {
    this.busyHosts.add(host);
    try {
      while (!this.stopped) {
        const queue = this.queues.get(host);
        const target = queue?.shift();
        if (target === undefined) break;

        const backoff = this.backoffs.get(host);
        if (backoff && backoff.until > this.now()) {
          // Hold this host's whole queue, not just this feed, and let every
          // other host carry on in the meantime.
          queue?.unshift(target);
          await this.sleep(backoff.until - this.now() + 1);
          if (this.stopped) break;
          continue;
        }
        // The gap is measured from the end of this host's previous request, so
        // a chain that started after a pause still spaces its requests.
        const last = this.lastRequestAt.get(host);
        const wait = last === undefined ? 0 : HOST_GAP_MS - (this.now() - last);
        if (wait > 0) await this.sleep(wait);
        if (this.stopped) break;

        const result = await this.fetchOne(target);
        this.lastRequestAt.set(host, this.now());

        // A result that landed is kept even when the reader stopped meanwhile:
        // the feed was fetched, so its entries are in the library.
        if (result.ok) {
          this.backoffs.delete(host);
          this.record(result);
        } else if (isRateLimited(result.status) && this.canRetry(host)) {
          // The refusal belongs to the host, not to this feed: hold the whole
          // queue and try again once the delay has passed.
          const refusals = (this.backoffs.get(host)?.refusals ?? 0) + 1;
          const delay = backoffFor(refusals);
          this.backoffs.set(host, { refusals, until: this.now() + delay });
          queue?.unshift(target);
          this.report();
          break;
        } else {
          // Either a broken feed, or a host that has refused as many times as
          // the run is willing to wait for.
          this.backoffs.delete(host);
          this.record(result);
        }
      }
    } finally {
      this.busyHosts.delete(host);
      if ((this.queues.get(host)?.length ?? 0) === 0) this.queues.delete(host);
      this.pump();
    }
  }

  /** Whether this host still has retries left in this run. */
  private canRetry(host: string): boolean {
    return (this.backoffs.get(host)?.refusals ?? 0) < RATE_LIMIT_DELAYS_MS.length;
  }

  private async fetchOne(target: RefreshTarget): Promise<FeedResult> {
    try {
      const outcome = await this.deps.refreshFeed(target.feedId);
      return {
        feedId: target.feedId,
        ok: outcome.error === null,
        newEntries: outcome.new_entries,
        error: outcome.error,
        status: outcome.http_status,
      };
    } catch (e) {
      // A command that never ran (the feed was removed mid-run, the bridge
      // failed) is a failed feed, not a failed run.
      return {
        feedId: target.feedId,
        ok: false,
        newEntries: 0,
        error: String(e),
        status: null,
      };
    }
  }

  private record(result: FeedResult) {
    this.results.push(result);
    this.report();
  }

  private report() {
    const progress: RefreshProgress = {
      total: this.covered.size,
      done: this.results.length,
      results: [...this.results],
      backoffs: this.currentBackoffs(),
      stopping: this.stopped,
    };
    this.deps.onProgress?.(progress);
  }

  private currentBackoffs(): HostBackoff[] {
    const waiting = new Map<string, number>();
    for (const [host, queue] of this.queues) waiting.set(host, queue.length);
    return [...this.backoffs.entries()]
      .filter(([host, entry]) => entry.until > this.now() && (waiting.get(host) ?? 0) > 0)
      .map(([host, entry]) => ({
        host,
        waiting: waiting.get(host) ?? 0,
        until: entry.until,
      }));
  }

  private finish() {
    if (this.finished) return;
    this.finished = true;
    const failed = this.results.filter((result) => !result.ok).length;
    this.deps.onDone?.({
      total: this.covered.size,
      ok: this.results.length - failed,
      failed,
      newEntries: this.results.reduce((n, result) => n + result.newEntries, 0),
      stopped: this.stopped,
      results: [...this.results],
    });
  }

}

/**
 * The one runner the application uses. Every trigger reaches the same run: a
 * second `start` while one is in flight extends it instead of competing with it.
 */
export function createRefreshRunner(deps: RefreshDeps): RefreshRunner {
  let live: Run | null = null;

  return {
    isRunning: () => live !== null && live.isRunning,
    start(targets) {
      const startingFresh = live === null || !live.isRunning;
      if (startingFresh) live = new Run(deps);
      const added = live!.add(targets);
      // Extending a live run is not a new run: the panel keeps reporting the
      // one it is already showing.
      if (startingFresh && added > 0) deps.onStart?.(added);
    },
    cancel() {
      live?.cancel();
    },
  };
}
