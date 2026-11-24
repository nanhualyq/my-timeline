/**
 * Relative time formatting for the entry list.
 *
 * Kept pure — the caller supplies `now` — so the buckets can be tested without
 * faking the system clock, and so a list that stays open can advance its own
 * `now` instead of every row re-reading `Date.now()`.
 */

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Below this, "just now" reads better than "0m". */
const JUST_NOW = 45_000;
const WEEK = 7 * DAY;

/**
 * A short relative label for `iso` as of `now` (epoch ms).
 *
 * Buckets: "just now" under 45 seconds, minutes under an hour, hours under a
 * day, days under a week, and a short absolute date beyond that — a reader
 * scanning a year-old backlog cares about "Mar 4", not "311d". Future
 * timestamps (clock skew, scheduled posts) clamp to "just now" rather than
 * rendering a negative interval. An unparseable or missing value renders as
 * the empty string, so a row's meta line simply drops the label.
 */
export function formatRelativeTime(
  iso: string | null | undefined,
  now: number,
): string {
  if (!iso) return "";
  const then = new Date(iso).getTime();
  if (isNaN(then)) return "";

  const elapsed = now - then;
  if (elapsed < JUST_NOW) return "just now";
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)}m`;
  if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)}h`;
  if (elapsed < WEEK) return `${Math.floor(elapsed / DAY)}d`;
  return new Date(then).toLocaleDateString();
}

/** Full timestamp for a row's `title` attribute: the escape hatch from the
 *  relative label, without opening the entry. */
export function fullTimestamp(iso: string | null | undefined): string {
  if (!iso) return "";
  const then = new Date(iso);
  return isNaN(then.getTime()) ? "" : then.toLocaleString();
}
