// Dates in plain words, in one place, so no surface prints a bare `Oct 12 · 3d` or an ISO date and
// every surface says one date the same way: "Due in 3 days", "3 days late", "Decide by Oct 14",
// "in 18 days (Nov 23)". A date more than two weeks off is a date: the distance is only worth a word
// while it is near. Pure, so the unit tests check it without a page.
import { dayOf } from "../timeline/model.ts";
import { plural } from "../ui/words.ts";

/** Days a date is called by its distance rather than by its name. */
export const NEAR_DAYS = 14;

/** A date this near (in days) is a soon date, said in warning ink (DESIGN, Dates). */
export const SOON_DAYS = 7;

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** The wall time (`08:15`) of a moment (`2026-10-10T08:15+01:00`), in the deployment's zone; none for a day. */
export function timeOf(when: string): string | undefined {
  return when.length > 10 ? when.slice(11, 16) : undefined;
}

/** ` at 08:15` after a distance in words (`Due tomorrow at 08:15`); nothing for a day. */
function atTime(when: string): string {
  const time = timeOf(when);
  return time === undefined ? "" : ` at ${time}`;
}

/** Which of two days or moments is earlier: by day, a day before any moment on it, then by instant (never by text). */
export function compareWhen(left: string, right: string): number {
  return dayOf(left) - dayOf(right) || Number(timeOf(left) !== undefined) - Number(timeOf(right) !== undefined) || Date.parse(left) - Date.parse(right) || 0;
}

/** A calendar date in words: `Oct 6`, with the year (`Oct 6, 2027`) when it is not in `today`'s, or always when there is no `today` to compare with; a moment adds its time (`Oct 6, 08:15`). */
export function dateWords(date: string, today?: string): string {
  const parsed = new Date(`${date.slice(0, 10)}T00:00:00Z`);
  const words = `${MONTHS[parsed.getUTCMonth()] ?? ""} ${String(parsed.getUTCDate())}`;
  const day = today !== undefined && date.slice(0, 4) === today.slice(0, 4) ? words : `${words}, ${String(parsed.getUTCFullYear())}`;
  const time = timeOf(date);
  return time === undefined ? day : `${day}, ${time}`;
}

/**
 * A moment's day and time of day in `zone` as the engine stores it: the wall time and the zone's UTC offset then
 * (`2026-10-10T08:15+01:00`). A wall time that happens twice (the clocks go back) reads as its first time, and one that
 * is skipped (they go forward) as the time that many minutes on.
 */
export function momentIn(zone: string, day: string, time: string): string {
  const wall = Date.parse(`${day}T${time}:00Z`);
  const offsetAt = (instant: number): number => {
    const parts = new Intl.DateTimeFormat("en-CA", { timeZone: zone, hourCycle: "h23", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit" }).formatToParts(new Date(instant));
    const field = (type: Intl.DateTimeFormatPartTypes) => Number(parts.find((part) => part.type === type)?.value);
    const shown = Date.UTC(field("year"), field("month") - 1, field("day"), field("hour"), field("minute"), field("second"));
    return Math.round((shown - Math.floor(instant / 1000) * 1000) / 60_000);
  };
  const DAY = 86_400_000;
  // The offsets either side of a change; each that reads the wall time as written holds.
  const around = [offsetAt(wall - DAY), offsetAt(wall + DAY)];
  const holding = around.filter((minutes) => offsetAt(wall - minutes * 60_000) === minutes);
  const instant = wall - (holding.length > 0 ? Math.max(...holding) : Math.min(...around)) * 60_000;
  const minutes = offsetAt(instant);
  const shown = new Date(instant + minutes * 60_000).toISOString();
  const pad = (count: number) => String(Math.abs(count)).padStart(2, "0");
  return `${shown.slice(0, 10)}T${shown.slice(11, 16)}${minutes < 0 ? "-" : "+"}${pad(Math.trunc(minutes / 60))}:${pad(minutes % 60)}`;
}

/** The text of a lateness in days: a time that has passed today is late by no days. */
export function lateWords(days: number): string {
  return days === 0 ? "late today" : `${daysWords(days)} late`;
}

/** A count of days in words: "1 day", "3 days". */
export function daysWords(days: number): string {
  return plural(days, "day");
}

/** Whole days from `today` to `date`; negative once it has passed. */
export function daysUntil(date: string, today: string): number {
  return dayOf(date) - dayOf(today);
}

/** Whether `date` is today or within the soon window ahead of `today` (never one already gone by). */
export function isSoon(date: string, today: string): boolean {
  const away = daysUntil(date, today);
  return away >= 0 && away <= SOON_DAYS;
}

/** The ink a date is said in (DESIGN, Dates): error once it has gone by, warning while it is soon, none after; the class is `ink-bad` or `ink-warn`. */
export function dateInk(date: string, today: string): "bad" | "warn" | undefined {
  return daysUntil(date, today) < 0 ? "bad" : isSoon(date, today) ? "warn" : undefined;
}

/** The class that says a date's ink, none when it has none. */
export function inkClass(ink: "bad" | "warn" | undefined): string | undefined {
  return ink === undefined ? undefined : `ink-${ink}`;
}

/** A moment in days, in words: "today", "tomorrow", "in 5 days", "yesterday", "3 days ago". */
export function relativeDays(days: number): string {
  if (days === 0) {
    return "today";
  }
  if (days === 1) {
    return "tomorrow";
  }
  return days > 0 ? `in ${daysWords(days)}` : days === -1 ? "yesterday" : `${daysWords(-days)} ago`;
}

/**
 * When a node is due, in words: late once it has passed, a distance while it is near, a date
 * after that. A decision is decided by a date, not due on it.
 */
export function dueWords(date: string, today: string, decision = false): string {
  const days = daysUntil(date, today);
  if (days < 0) {
    return `${daysWords(-days)} late`;
  }
  const at = atTime(date);
  if (decision) {
    return days === 0 ? `Decide today${at}` : `Decide by ${dateWords(date, today)}`;
  }
  if (days === 0) {
    return `Due today${at}`;
  }
  if (days === 1) {
    return `Due tomorrow${at}`;
  }
  return days <= NEAR_DAYS ? `Due in ${daysWords(days)}${at}` : `Due ${dateWords(date, today)}`;
}

/** Where an effective date comes from: a recorded actual, a pin, or worked out from the plan (the engine's `DateOrigin`, or the inspector's bound origin). */
export type Origin = "actual" | "pin" | "due" | "derived";

/** The one word for where a date comes from; an actual date has none on a summary (it says "done"). */
export function originWord(origin: Origin): string {
  switch (origin) {
    case "actual":
      return "recorded";
    case "pin":
      return "pinned";
    case "due":
    case "derived":
      return "worked out";
  }
}

/**
 * A milestone's date as a reader weighs it, the nearness first and the calendar date after it, so
 * it reads right after a verb title or a noun title: `in 18 days (Nov 23)`, `3 days late (Oct 2)`,
 * `today (Oct 6)`; an actual date is `done Nov 20` (never late).
 */
export function whenWords(date: string, origin: Origin | undefined, today: string): string {
  const when = dateWords(date, today);
  if (origin === "actual") {
    return `done ${when}`;
  }
  const days = daysUntil(date, today);
  return days < 0 ? `${daysWords(-days)} late (${when})` : `${relativeDays(days)} (${when})`;
}
