// C13: the timeline as data. The engine's projection places each in-scope milestone at its
// effective date (actual, pin, or derived due), each other pinned node at its pin, and open
// work at its due date, earliest first, with overdue and shortfall marked and the `final`
// milestone named as the end anchor. This lays those dates on an axis: its span (today and
// every date, with a margin), its ticks at a granularity that keeps them few, the end anchor,
// and where each day falls along it as a fraction of its width. A route's timeline lays days
// from the journey's start on the same axis, relative: ticks on multiples of a step of whole
// days, the start where a journey's has today. The axis works in day numbers (days since
// 1970-01-01 for a journey, days from the start for a route), so nothing downstream can show a
// made-up calendar date. Pure, so the unit tests check it without a page.
//
// Cost: O(entries) per timeline, plus O(ticks), which `TICK_COUNT_MAX` bounds.
import type { Schema } from "@cairn/client";
import type { RouteSchedule } from "@cairn/wasm";

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

/** A calendar date in words: `Oct 6`, with the year (`Oct 6, 2027`) when it is not in `today`'s, or always when there is no `today` to compare with. */
export function dateWords(date: string, today?: string): string {
  const parsed = new Date(`${date}T00:00:00Z`);
  const words = `${MONTHS[parsed.getUTCMonth()] ?? ""} ${String(parsed.getUTCDate())}`;
  return today !== undefined && date.slice(0, 4) === today.slice(0, 4) ? words : `${words}, ${String(parsed.getUTCFullYear())}`;
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
  day: number;
  label: string;
  /** Where it falls, from 0 (the axis's start) to 1 (its end). */
  at: number;
}

/** C13: the end anchor, the journey's `final` milestone at its day. */
export interface Anchor {
  node: string;
  day: number;
}

/** The days an axis shows, from the first to the last. */
export interface Window {
  first: number;
  last: number;
}

/** The range control's choices: every date, a month from today, a quarter from today. */
export const RANGES = ["fit", "month", "quarter"] as const;
export type Range = (typeof RANGES)[number];

/** Days of today's past each fixed range keeps in view, and its length. */
const RANGE_DAYS = { month: { before: 3, length: 31 }, quarter: { before: 7, length: 92 } } as const;

/** The window `range` shows around `today`; none for Fit, which is every date. */
export function rangeWindow(range: Range, today: string): Window | undefined {
  if (range === "fit") {
    return undefined;
  }
  const first = dayOf(today) - RANGE_DAYS[range].before;
  return { first, last: first + RANGE_DAYS[range].length };
}

/** How an axis names its days: calendar dates (a journey's), or days from the start (a route's). */
export type AxisMode = "calendar" | "relative";

/** C13: the time axis the timeline's days lie on. */
export interface Axis {
  mode: AxisMode;
  /** The first and last day shown. */
  start: number;
  end: number;
  /** The end anchor; none when there is no `final` milestone in scope with a day. */
  anchor: Anchor | undefined;
  unit: TickUnit;
  ticks: Tick[];
  /** Where today falls on a calendar axis, the start (day 0) on a relative one. */
  now: number;
}

/** What an axis is laid out from: how it names days, the day it marks, the end anchor, and every day the data lies on. */
export interface AxisSpec {
  mode: AxisMode;
  /** The day marked: today, or day 0. */
  now: number;
  anchor: Anchor | undefined;
  days: readonly number[];
}

/**
 * C13: the end anchor: the `final` milestone the projection names, at its day. A journey
 * with no final milestone, or whose final milestone has no date yet, has none: the axis then
 * simply ends after its latest date.
 */
export function endAnchor(timeline: Timeline): Anchor | undefined {
  const entry = timeline.entries.find((each) => each.node === timeline.end);
  return entry === undefined ? undefined : { node: entry.node, day: dayOf(entry.date) };
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
    day,
    label: labelOf(day, unit, index === 0),
    at: (day - first) / (last - first),
  }));
}

/** The steps (days) a relative axis ticks by, finest first: whole days, weeks, then months and longer. */
const RELATIVE_STEPS = [1, 2, 5, 7, 10, 14, 30, 60, 90, 180, 365] as const;

/** The finest relative step that keeps a span of `spanDays` within `TICK_COUNT_MAX` ticks; years beyond. */
export function relativeStepFor(spanDays: number): number {
  return RELATIVE_STEPS.find((step) => spanDays / step <= TICK_COUNT_MAX) ?? 365 * Math.ceil(spanDays / 365 / TICK_COUNT_MAX);
}

/** Day `day` of a journey that starts on day 0, in words: "Day 7". */
export function dayWords(day: number): string {
  return `Day ${String(day)}`;
}

/** The ticks of a relative axis: the multiples of the step that lie from day `first` to day `last`. */
function relativeTicksOf(first: number, last: number): Tick[] {
  const step = relativeStepFor(last - first);
  const ticks: Tick[] = [];
  for (let day = Math.ceil(first / step) * step; day <= last; day += step) {
    ticks.push({ day, label: dayWords(day), at: (day - first) / (last - first) });
  }
  return ticks;
}

/**
 * C13: the axis `spec` lays out: its marked day and every day (`days` beside `extra`, which the
 * bars add) with a margin; or `window`, the days a range or a pan and zoom picked.
 */
export function axisOf(spec: AxisSpec, extra: readonly number[] = [], window?: Window): Axis {
  const days = [spec.now, ...spec.days, ...extra];
  const first = window?.first ?? Math.min(...days) - AXIS_MARGIN_DAYS;
  const last = window?.last ?? Math.max(...days) + AXIS_MARGIN_DAYS;
  const unit = spec.mode === "calendar" ? unitFor(last - first) : "day";
  return {
    mode: spec.mode,
    start: first,
    end: last,
    anchor: spec.anchor,
    unit,
    ticks: spec.mode === "calendar" ? ticksOf(first, last, unit) : relativeTicksOf(first, last),
    now: (spec.now - first) / (last - first),
  };
}

/** C13: what a journey's axis is laid out from: today, the timeline's dates and its end anchor. */
export function calendarSpec(timeline: Timeline, today: string): AxisSpec {
  return { mode: "calendar", now: dayOf(today), anchor: endAnchor(timeline), days: timeline.entries.map((entry) => dayOf(entry.date)) };
}

/** C13: what a route's axis is laid out from: day 0 as its start, the schedule's days and its final milestone as the end anchor, at the day it can finish. */
export function routeSpec(schedule: RouteSchedule): AxisSpec {
  const end = schedule.entries.find((entry) => entry.node === schedule.end);
  return {
    mode: "relative",
    now: 0,
    anchor: end === undefined ? undefined : { node: end.node, day: end.finish_day },
    days: schedule.entries.flatMap((entry) => [entry.start_day, entry.finish_day]),
  };
}

/** C13: the axis for `timeline` read on `today`, with the days `dates` beside the timeline's own. */
export function timelineAxis(timeline: Timeline, today: string, dates: readonly number[] = [], window?: Window): Axis {
  return axisOf(calendarSpec(timeline, today), dates, window);
}

/** Where `day` falls along `axis`, from 0 to 1. */
export function positionOf(axis: Axis, day: number): number {
  return (day - axis.start) / (axis.end - axis.start);
}
