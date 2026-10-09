// C13: the timeline as data. The engine's projection places each in-scope milestone at its
// effective date (actual, pin, or derived due), each other pinned node at its pin, and open
// work at its due date, earliest first, with overdue and shortfall marked and the `final`
// milestone named as the end anchor. This lays those dates on an axis: its span (today and
// every date, with a margin), its ticks at a granularity that keeps them few, the end anchor,
// and where each date falls along it as a fraction of its width. Pure, so the unit tests
// check it without a page.
//
// Cost: O(entries) per timeline, plus O(ticks), which `TICK_COUNT_MAX` bounds.
import type { Schema } from "@cairn/client";

import { titleOf, type NodeKind, type Ready } from "../detail/model.ts";

export type Timeline = Schema<"Timeline">;
export type TimelineEntry = Schema<"TimelineEntry">;
export type DateOrigin = Schema<"DateOrigin">;

const DAY_MS = 86_400_000;

/** Days of margin the axis keeps before its first date and after its last. */
export const AXIS_MARGIN_DAYS = 2;

/** The most ticks the axis draws; the granularity is the finest that keeps within it. */
export const TICK_COUNT_MAX = 12;

/** The axis's granularities, finest first, each with the days one step spans at most. */
const UNITS = [
  { unit: "day", days: 1 },
  { unit: "week", days: 7 },
  { unit: "month", days: 31 },
  { unit: "quarter", days: 92 },
  { unit: "year", days: 366 },
] as const;

export type TickUnit = (typeof UNITS)[number]["unit"];

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** A calendar date in words: `Oct 6`, with the year (`Oct 6, 2027`) when it is not in `today`'s. */
export function dateWords(date: string, today: string): string {
  const parsed = new Date(`${date}T00:00:00Z`);
  const words = `${MONTHS[parsed.getUTCMonth()] ?? ""} ${String(parsed.getUTCDate())}`;
  return date.slice(0, 4) === today.slice(0, 4) ? words : `${words}, ${String(parsed.getUTCFullYear())}`;
}

/** A date in words with how far off it is from `today`: `Oct 30, in 24 days`, `Oct 2, 4 days late`, `Oct 6, today`. */
export function dateAway(date: string, today: string): string {
  const days = dayOf(date) - dayOf(today);
  const count = (n: number) => `${String(n)} ${n === 1 ? "day" : "days"}`;
  return `${dateWords(date, today)}, ${days < 0 ? `${count(-days)} late` : days === 0 ? "today" : `in ${count(days)}`}`;
}

/** A calendar date (`YYYY-MM-DD`) as days since 1970-01-01. */
export function dayOf(date: string): number {
  const parsed = Date.parse(`${date}T00:00:00Z`);
  if (Number.isNaN(parsed)) {
    throw new Error(`not a calendar date: ${date}`);
  }
  return Math.round(parsed / DAY_MS);
}

/** Days since 1970-01-01 as a calendar date (`YYYY-MM-DD`). */
export function dateOf(day: number): string {
  return new Date(day * DAY_MS).toISOString().slice(0, 10);
}

/** One labeled tick on the axis. */
export interface Tick {
  date: string;
  label: string;
  /** Where it falls, from 0 (the axis's start) to 1 (its end). */
  at: number;
}

/** C13: the end anchor, the journey's `final` milestone at its date. */
export interface Anchor {
  node: string;
  date: string;
}

/** C13: the time axis the timeline's dates lie on. */
export interface Axis {
  start: string;
  end: string;
  /** The end anchor; none when the journey has no `final` milestone in scope with a date. */
  anchor: Anchor | undefined;
  unit: TickUnit;
  ticks: Tick[];
  /** Where today falls. */
  today: number;
}

/**
 * C13: the end anchor: the `final` milestone the projection names, at its date. A journey
 * with no final milestone, or whose final milestone has no date yet, has none: the axis then
 * simply ends after its latest date.
 */
export function endAnchor(timeline: Timeline): Anchor | undefined {
  const entry = timeline.entries.find((each) => each.node === timeline.end);
  return entry === undefined ? undefined : { node: entry.node, date: entry.date };
}

/** The finest step that keeps a span of `spanDays` within `TICK_COUNT_MAX` ticks; years beyond. */
function unitFor(spanDays: number): TickUnit {
  return UNITS.find((each) => spanDays / each.days <= TICK_COUNT_MAX)?.unit ?? "year";
}

/** Months one step of a calendar unit spans; a year's step widens so the ticks stay few. */
function monthsPerStep(unit: "month" | "quarter" | "year", spanDays: number): number {
  switch (unit) {
    case "month":
      return 1;
    case "quarter":
      return 3;
    case "year":
      return 12 * Math.max(1, Math.ceil(spanDays / 366 / TICK_COUNT_MAX));
  }
}

/**
 * The days from `first` to `last` that start a step of `unit`: every day, every Monday, or
 * the first of every month, quarter, or year (of every few years on a very long span).
 */
function stepDays(first: number, last: number, unit: TickUnit): number[] {
  const days: number[] = [];
  if (unit === "day" || unit === "week") {
    const step = unit === "day" ? 1 : 7;
    // 1970-01-01 was a Thursday: day 4 is the first Monday.
    const start = unit === "day" ? first : first + ((((4 - first) % 7) + 7) % 7);
    for (let day = start; day <= last; day += step) {
      days.push(day);
    }
    return days;
  }
  const months = monthsPerStep(unit, last - first);
  const from = new Date(first * DAY_MS);
  const index = from.getUTCFullYear() * 12 + from.getUTCMonth();
  for (let month = Math.ceil(index / months) * months; ; month += months) {
    const day = Math.round(Date.UTC(Math.floor(month / 12), month % 12, 1) / DAY_MS);
    if (day > last) {
      return days;
    }
    if (day >= first) {
      days.push(day);
    }
  }
}

function labelOf(day: number, unit: TickUnit, first: boolean): string {
  const date = new Date(day * DAY_MS);
  const month = MONTHS[date.getUTCMonth()] ?? "";
  const year = String(date.getUTCFullYear());
  switch (unit) {
    case "day":
    case "week":
      return `${month} ${String(date.getUTCDate())}`;
    case "month":
    case "quarter":
      return first || date.getUTCMonth() === 0 ? `${month} ${year}` : month;
    case "year":
      return year;
  }
}

/** The ticks from day `first` to day `last` at `unit`, each placed along the span. */
function ticksOf(first: number, last: number, unit: TickUnit): Tick[] {
  return stepDays(first, last, unit).map((day, index) => ({
    date: dateOf(day),
    label: labelOf(day, unit, index === 0),
    at: (day - first) / (last - first),
  }));
}

/** C13: the axis for `timeline` read on `today`: today and every date, with a margin. */
export function timelineAxis(timeline: Timeline, today: string): Axis {
  const days = [today, ...timeline.entries.map((entry) => entry.date)].map(dayOf);
  const first = Math.min(...days) - AXIS_MARGIN_DAYS;
  const last = Math.max(...days) + AXIS_MARGIN_DAYS;
  const unit = unitFor(last - first);
  return {
    start: dateOf(first),
    end: dateOf(last),
    anchor: endAnchor(timeline),
    unit,
    ticks: ticksOf(first, last, unit),
    today: (dayOf(today) - first) / (last - first),
  };
}

/** Where `date` falls along `axis`, from 0 to 1. */
export function positionOf(axis: Axis, date: string): number {
  const first = dayOf(axis.start);
  return (dayOf(date) - first) / (dayOf(axis.end) - first);
}

/** C13, F7: one date on the timeline, told apart as actual, pin, or derived due. */
export interface TimelineRow {
  node: string;
  title: string;
  kind: NodeKind;
  date: string;
  origin: DateOrigin;
  overdue: boolean;
  shortfallDays: number | undefined;
  /** The end anchor. */
  end: boolean;
  /** Where it falls along the axis. */
  at: number;
}

/** What the toolbar narrows the timeline to: DECISIONS, the kinds the filter holds, and the search text. */
export interface TimelineNarrowing {
  decisions: boolean;
  kinds: readonly NodeKind[];
  text: string;
}

/** Whether a node of `kind` and `title` stays on the timeline under `narrowing`. */
export function timelineKeeps(narrowing: TimelineNarrowing, kind: NodeKind, title: string): boolean {
  const wanted = narrowing.text.trim().toLowerCase();
  return (
    (narrowing.decisions ? kind === "decision" : narrowing.kinds.length === 0 || narrowing.kinds.includes(kind)) &&
    (wanted === "" || title.toLowerCase().includes(wanted))
  );
}

/** C13: the timeline's dates in its order (earliest first), placed on `axis`. */
export function timelineRows(ready: Ready, timeline: Timeline, axis: Axis): TimelineRow[] {
  return timeline.entries.map((entry) => ({
    node: entry.node,
    title: titleOf(ready, entry.node),
    kind: entry.kind,
    date: entry.date,
    origin: entry.origin,
    overdue: entry.overdue === true,
    shortfallDays: entry.shortfall_days ?? undefined,
    end: entry.node === axis.anchor?.node,
    at: positionOf(axis, entry.date),
  }));
}
