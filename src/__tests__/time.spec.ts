import { describe, expect, it } from "vitest";
import { formatRelativeTime, fullTimestamp } from "../lib/time";

const NOW = Date.parse("2026-03-10T12:00:00Z");

function ago(ms: number): string {
  return new Date(NOW - ms).toISOString();
}

describe("formatRelativeTime", () => {
  it("says just now under 45 seconds", () => {
    expect(formatRelativeTime(ago(0), NOW)).toBe("just now");
    expect(formatRelativeTime(ago(44_000), NOW)).toBe("just now");
  });

  it("counts minutes below an hour", () => {
    expect(formatRelativeTime(ago(45_000), NOW)).toBe("0m");
    expect(formatRelativeTime(ago(20 * 60_000), NOW)).toBe("20m");
    expect(formatRelativeTime(ago(59 * 60_000), NOW)).toBe("59m");
  });

  it("counts hours below a day", () => {
    expect(formatRelativeTime(ago(60 * 60_000), NOW)).toBe("1h");
    expect(formatRelativeTime(ago(23 * 3_600_000), NOW)).toBe("23h");
  });

  it("counts days below a week", () => {
    expect(formatRelativeTime(ago(24 * 3_600_000), NOW)).toBe("1d");
    expect(formatRelativeTime(ago(6 * 24 * 3_600_000), NOW)).toBe("6d");
  });

  it("falls back to an absolute date past a week", () => {
    const label = formatRelativeTime(ago(8 * 24 * 3_600_000), NOW);
    expect(label).not.toMatch(/^\d+d$/);
    expect(label).toBe(new Date(NOW - 8 * 24 * 3_600_000).toLocaleDateString());
  });

  it("clamps future timestamps to just now", () => {
    expect(formatRelativeTime(new Date(NOW + 3_600_000).toISOString(), NOW)).toBe(
      "just now",
    );
  });

  it("renders nothing for a missing or unparseable time", () => {
    expect(formatRelativeTime(null, NOW)).toBe("");
    expect(formatRelativeTime("not a date", NOW)).toBe("");
  });
});

describe("fullTimestamp", () => {
  it("renders the full locale timestamp", () => {
    const iso = "2026-03-10T09:30:00Z";
    expect(fullTimestamp(iso)).toBe(new Date(iso).toLocaleString());
  });

  it("renders nothing when there is no time", () => {
    expect(fullTimestamp(null)).toBe("");
  });
});
