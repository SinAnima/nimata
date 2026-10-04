import type { UnixMillis } from "./types";

const MINUTE = 60_000;
const DAY = 24 * 60 * MINUTE;

function startOfLocalDay(ms: UnixMillis): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Short, local, human time for stream headers and the sidebar. */
export function friendlyTime(at: UnixMillis, now: UnixMillis): string {
  const time = new Date(at).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
  const days = Math.round((startOfLocalDay(now) - startOfLocalDay(at)) / DAY);
  if (days === 0) return time;
  if (days === 1) return `Yesterday ${time}`;
  const sameYear = new Date(at).getFullYear() === new Date(now).getFullYear();
  const date = new Date(at).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  });
  return days < 7
    ? `${new Date(at).toLocaleDateString(undefined, { weekday: "short" })} ${time}`
    : date;
}

export function formatOffset(minutes: number): string {
  const sign = minutes < 0 ? "-" : "+";
  const abs = Math.abs(minutes);
  const hh = String(Math.floor(abs / 60)).padStart(2, "0");
  const mm = String(abs % 60).padStart(2, "0");
  return `UTC${sign}${hh}:${mm}`;
}

/**
 * The exact instant as the author's wall clock showed it, e.g.
 * "2026-10-04 12:41:07 UTC-04:00". Independent of the viewer's time zone.
 */
export function authorWallClock(at: UnixMillis, offsetMinutes: number): string {
  const shifted = new Date(at + offsetMinutes * MINUTE).toISOString();
  return `${shifted.slice(0, 10)} ${shifted.slice(11, 19)} ${formatOffset(offsetMinutes)}`;
}

export function utcIso(at: UnixMillis): string {
  return new Date(at).toISOString();
}

export type DayGroup = "Today" | "Yesterday" | "Earlier";

export function dayGroup(at: UnixMillis, now: UnixMillis): DayGroup {
  const days = Math.round((startOfLocalDay(now) - startOfLocalDay(at)) / DAY);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  return "Earlier";
}
