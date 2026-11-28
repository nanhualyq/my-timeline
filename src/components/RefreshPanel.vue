<script setup lang="ts">
import { computed } from "vue";
import type {
  FeedResult,
  RefreshPanelState,
  RefreshProgress,
  RefreshSummary,
} from "../lib/refresh";

/**
 * The refresh panel.
 *
 * A run is a notice, not a dialog: it never blocks the reader, and it is only
 * shown once there is something to say — a run long enough to watch, or a
 * summary that outlives it. A clean summary withdraws on its own; one with
 * failures stays until the reader dismisses it.
 */
const props = defineProps<{
  state: RefreshPanelState;
  /** Display titles by feed id, so a row names the feed the reader knows. */
  titles: Record<number, string>;
}>();

const emit = defineEmits<{
  stop: [];
  retry: [];
  dismiss: [];
}>();

const running = computed<RefreshProgress | null>(() =>
  props.state.phase === "running" ? props.state.progress : null,
);
const finished = computed<RefreshSummary | null>(() =>
  props.state.phase === "done" ? props.state.summary : null,
);

/** Feeds listed while the run is in flight: the newest few are enough to see
 *  what is happening, and the count carries the rest. */
const VISIBLE_ROWS = 4;

/** A feed's name, falling back to its id when the sidebar has none (the feed
 *  was removed mid-run). */
function nameOf(result: FeedResult): string {
  return props.titles[result.feedId] ?? `Feed ${result.feedId}`;
}

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

function runningRows(progress: RefreshProgress): FeedResult[] {
  return progress.results.slice(-VISIBLE_ROWS);
}

function failedRows(summary: RefreshSummary): FeedResult[] {
  return summary.results.filter((result) => !result.ok);
}

function summaryLine(summary: RefreshSummary): string {
  const parts = [
    plural(summary.total, "feed", "feeds"),
    `${summary.newEntries} new`,
  ];
  if (summary.failed > 0) parts.push(`${summary.failed} failed`);
  if (summary.stopped) parts.push("stopped");
  return parts.join(" · ");
}

function barWidth(progress: RefreshProgress): number {
  if (progress.total === 0) return 0;
  return (progress.done / progress.total) * 100;
}
</script>

<template>
  <section class="refresh-panel">
    <template v-if="running">
      <header class="panel-head">
        <span class="panel-title">{{ running.stopping ? "Stopping…" : "Refreshing" }}</span>
        <span class="panel-count">{{ running.done }} / {{ running.total }}</span>
        <button
          v-if="!running.stopping"
          type="button"
          class="panel-action"
          @click="emit('stop')"
        >
          Stop
        </button>
      </header>
      <div
        class="panel-bar"
        role="progressbar"
        :aria-valuemin="0"
        :aria-valuemax="running.total"
        :aria-valuenow="running.done"
      >
        <span class="panel-bar-fill" :style="{ width: `${barWidth(running)}%` }"></span>
      </div>
      <p v-for="host in running.backoffs" :key="host.host" class="panel-backoff">
        {{ host.host }} is refusing requests · {{ plural(host.waiting, "feed", "feeds") }} waiting
      </p>
      <ul class="panel-rows">
        <li
          v-for="result in runningRows(running)"
          :key="result.feedId"
          class="panel-row"
          :class="{ failed: !result.ok }"
        >
          <span class="panel-mark" aria-hidden="true">{{ result.ok ? "✓" : "×" }}</span>
          <span class="panel-row-name">{{ nameOf(result) }}</span>
          <span v-if="result.ok && result.newEntries > 0" class="panel-row-detail">
            +{{ result.newEntries }}
          </span>
          <span v-else-if="!result.ok" class="panel-row-detail">{{ result.error }}</span>
        </li>
      </ul>
    </template>

    <template v-else-if="finished">
      <header class="panel-head">
        <span class="panel-title">
          {{ finished.stopped ? "Refresh stopped" : "Refresh complete" }}
        </span>
        <button
          type="button"
          class="panel-action"
          aria-label="Dismiss refresh report"
          @click="emit('dismiss')"
        >
          ×
        </button>
      </header>
      <!-- The one thing announced: a counter read out on every feed would flood
           assistive technology. -->
      <p class="panel-summary" role="status" aria-live="polite">
        {{ summaryLine(finished) }}
      </p>
      <ul v-if="finished.failed > 0" class="panel-rows">
        <li v-for="result in failedRows(finished)" :key="result.feedId" class="panel-row failed">
          <span class="panel-mark" aria-hidden="true">×</span>
          <span class="panel-row-name">{{ nameOf(result) }}</span>
          <span class="panel-row-detail">{{ result.error }}</span>
        </li>
      </ul>
      <div class="panel-foot">
        <button
          v-if="finished.failed > 0"
          type="button"
          class="panel-action"
          @click="emit('retry')"
        >
          Retry {{ plural(finished.failed, "failed feed", "failed feeds") }}
        </button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.refresh-panel {
  min-width: min(20rem, calc(100vw - 2rem));
  max-width: min(28rem, calc(100vw - 2rem));
  padding: 0.5rem 0.6rem;
  background: #2f2f33;
  color: #f5f5f5;
  border-radius: 6px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.28);
  font-size: 0.85rem;
}
.panel-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.panel-title {
  font-weight: 600;
}
.panel-count {
  margin-left: auto;
  font-variant-numeric: tabular-nums;
  opacity: 0.85;
}
.panel-action {
  border: 1px solid rgba(255, 255, 255, 0.35);
  background: transparent;
  color: inherit;
  border-radius: 4px;
  padding: 0.2rem 0.5rem;
  cursor: pointer;
  font-size: 0.8rem;
}
.panel-action:active {
  background: rgba(255, 255, 255, 0.12);
}
@media (hover: hover) and (pointer: fine) {
  .panel-action:hover {
    background: rgba(255, 255, 255, 0.12);
  }
}
.panel-bar {
  height: 3px;
  margin: 0.4rem 0;
  background: rgba(255, 255, 255, 0.18);
  border-radius: 2px;
  overflow: hidden;
}
.panel-bar-fill {
  display: block;
  height: 100%;
  background: #7fb0ff;
  transition: width 120ms linear;
}
.panel-backoff {
  margin: 0.25rem 0;
  color: #ffd479;
}
.panel-summary {
  margin: 0.35rem 0 0;
}
.panel-rows {
  list-style: none;
  margin: 0.35rem 0 0;
  padding: 0;
}
.panel-row {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  padding: 0.1rem 0;
  white-space: nowrap;
  overflow: hidden;
}
.panel-mark {
  color: #8fdca0;
  flex-shrink: 0;
}
.panel-row.failed .panel-mark {
  color: #ff9d92;
}
.panel-row-name {
  flex-shrink: 0;
}
.panel-row-detail {
  overflow: hidden;
  text-overflow: ellipsis;
  opacity: 0.8;
}
.panel-foot {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.4rem;
}
</style>
