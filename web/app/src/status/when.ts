// Dates in plain words, in one place, so no surface prints a bare `Oct 12 · 3d` or an ISO date and
// every surface says one date the same way: "Due in 3 days", "3 days late", "Decide by Oct 14",
// "in 18 days (Nov 23)". A date more than two weeks off is a date: the distance is only worth a word
// while it is near. A day is a calendar day everywhere; a moment is said on the viewer's clock, the
// browser's zone and hour cycle. Pure but for those two defaults, so the unit tests check it without a page.
import { dayOf } from "../timeline/model.ts";
import { plural } from "../ui/words.ts";

/** Days a date is called by its distance rather than by its name. */
export const NEAR_DAYS = 14;

/** A date this near (in days) is a soon date, said in warning ink (DESIGN, Dates). */
export const SOON_DAYS = 7;

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** The viewer's time zone and locale: the browser's. */
const VIEWER = new Intl.DateTimeFormat().resolvedOptions();

/** A clock reading: the calendar day (`2026-10-11`) and the 24-hour wall time (`04:00`). */
interface Wall {
  day: string;
  time: string;
}

/** The wall clock in `zone` at `instant` (ms since the epoch), to the minute. */
export function wallIn(zone: string, instant: number): Wall {
  const parts = new Intl.DateTimeFormat("en-CA", { timeZone: zone, hourCycle: "h23", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).formatToParts(instant);
  const field = (type: Intl.DateTimeFormatPartTypes) => parts.find((part) => part.type === type)?.value ?? "";
  return { day: `${field("year")}-${field("month")}-${field("day")}`, time: `${field("hour")}:${field("minute")}` };
}

/** A time of day as the viewer's browser writes one, 12- or 24-hour by its locale, in its zone: the one formatter for every time a screen shows. */
export function timeWords(at: number | Date, zone: string = VIEWER.timeZone, locale: string = VIEWER.locale): string {
  return new Intl.DateTimeFormat(locale, { timeStyle: "short", timeZone: zone }).format(at);
}

/** The viewer's date today (`2026-10-10`). */
export function viewerToday(): string {
  return wallIn(VIEWER.timeZone, Date.now()).day;
}

/** Whether `when` is a moment (`2026-10-10T08:15+01:00`) rather than a day. */
function isMoment(when: string): boolean {
  return when.length > 10;
}

/** The viewer's clock at a moment: its day and 24-hour time in `zone` (the viewer's), as a date and a time input hold them; none for a day, which is a calendar day everywhere. */
export function localWall(when: string, zone: string = VIEWER.timeZone): Wall | undefined {
  return isMoment(when) ? wallIn(zone, Date.parse(when)) : undefined;
}

/** A calendar date in words: `Oct 6`, with the year (`Oct 6, 2027`) when it is not in `today`'s, or always when there is no `today` to compare with; a moment is said on the viewer's date and clock (`Oct 6, 4:00 PM`). */
export function dateWords(date: string, today?: string): string {
  const day = localWall(date)?.day ?? date.slice(0, 10);
  const parsed = new Date(`${day}T00:00:00Z`);
  const words = `${MONTHS[parsed.getUTCMonth()] ?? ""} ${String(parsed.getUTCDate())}`;
  const said = today !== undefined && day.slice(0, 4) === today.slice(0, 4) ? words : `${words}, ${String(parsed.getUTCFullYear())}`;
  return isMoment(date) ? `${said}, ${timeWords(Date.parse(date))}` : said;
}

/** A time of day in either hour cycle: `04:00`, `4:00 AM`, `4:00 p.m.`. */
const TIME = String.raw`\d{1,2}:\d{2}(?:\s?[AaPp]\.?[Mm]\b\.?)?`;

/** A date as `dateWords` says it (`Oct 6`, `Oct 6, 2027`, with a time `Oct 6, 4:00 PM`), or a time of day alone. */
const DATE_OR_TIME = new RegExp(`\\b(?:${MONTHS.join("|")}) \\d{1,2}(?:, \\d{4})?(?:, ${TIME})?|\\b${TIME}`, "g");

/** `words` cut into its plain stretches and the dates and times in it (`date`), so a screen sets only those in mono (DESIGN, Dates). */
export function dateParts(words: string): { text: string; date: boolean }[] {
  const parts: { text: string; date: boolean }[] = [];
  let at = 0;
  for (const found of words.matchAll(DATE_OR_TIME)) {
    if (found.index > at) {
      parts.push({ text: words.slice(at, found.index), date: false });
    }
    parts.push({ text: found[0], date: true });
    at = found.index + found[0].length;
  }
  return at < words.length ? [...parts, { text: words.slice(at), date: false }] : parts;
}

/** The UTC offset `zone` has at `instant`, in minutes. */
function offsetAt(zone: string, instant: number): number {
  const wall = wallIn(zone, instant);
  return Math.round((Date.parse(`${wall.day}T${wall.time}:00Z`) - Math.floor(instant / 60_000) * 60_000) / 60_000);
}

/**
 * The value the engine stores for a day and time of day read on the clock of `viewer` (the viewer's zone): the same
 * instant as the wall time in `deployment` and that zone's UTC offset then (`2026-10-10T08:15+01:00`). A wall time that
 * happens twice (the clocks go back) reads as its first time, and one that is skipped (they go forward) as the time
 * that many minutes on.
 */
export function momentFrom(deployment: string, day: string, time: string, viewer: string = VIEWER.timeZone): string {
  const wall = Date.parse(`${day}T${time}:00Z`);
  const DAY = 86_400_000;
  // The viewer's offsets either side of a change; each that reads the wall time as written holds.
  const around = [offsetAt(viewer, wall - DAY), offsetAt(viewer, wall + DAY)];
  const holding = around.filter((minutes) => offsetAt(viewer, wall - minutes * 60_000) === minutes);
  const instant = wall - (holding.length > 0 ? Math.max(...holding) : Math.min(...around)) * 60_000;
  const minutes = offsetAt(deployment, instant);
  const stored = wallIn(deployment, instant);
  const pad = (count: number) => String(Math.abs(count)).padStart(2, "0");
  return `${stored.day}T${stored.time}${minutes < 0 ? "-" : "+"}${pad(Math.trunc(minutes / 60))}:${pad(minutes % 60)}`;
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

/** The ink a date is said in (DESIGN, Dates): error once the engine calls the node late (`late`, its days overdue), warning while the date is soon, none after; the class is `ink-bad` or `ink-warn`. */
export function dateInk(date: string, today: string, late: number | undefined): "bad" | "warn" | undefined {
  return late !== undefined ? "bad" : isSoon(date, today) ? "warn" : undefined;
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
 * When a node is due, in words: late once the engine says so (`late`, its days overdue), a
 * distance while it is near, a date after that. A decision is decided by a date, not due on it.
 * A moment is named by its date and time: a distance counts the deployment's days, which the
 * viewer's clock need not share.
 */
export function dueWords(date: string, today: string, decision: boolean, late: number | undefined): string {
  if (late !== undefined) {
    return lateWords(late);
  }
  const days = daysUntil(date, today);
  if (decision) {
    return days === 0 && !isMoment(date) ? "Decide today" : `Decide by ${dateWords(date, today)}`;
  }
  if (isMoment(date)) {
    return `Due ${dateWords(date, today)}`;
  }
  if (days === 0) {
    return "Due today";
  }
  if (days === 1) {
    return "Due tomorrow";
  }
  return days > 0 && days <= NEAR_DAYS ? `Due in ${daysWords(days)}` : `Due ${dateWords(date, today)}`;
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
 * `today (Oct 6)`; a moment is its date and time alone (`Nov 23, 9:30 AM`), as in `dueWords`; an
 * actual date is `done Nov 20` (never late). `late` is the engine's days overdue.
 */
export function whenWords(date: string, origin: Origin | undefined, today: string, late: number | undefined): string {
  const when = dateWords(date, today);
  if (origin === "actual") {
    return `done ${when}`;
  }
  if (late !== undefined) {
    return `${lateWords(late)} (${when})`;
  }
  return isMoment(date) ? when : `${relativeDays(daysUntil(date, today))} (${when})`;
}
