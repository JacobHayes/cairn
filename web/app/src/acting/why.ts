// C10: why an item ranks where it does. Rank is the documented blend (Priority): each term
// times its constant, so the parts sum to the rank and the largest part is the reason the
// item sits where it does. When the list is sorted by one signal instead, that signal is the
// reason, and the blend stays beside it so the trade-off is visible.
import type { Schema } from "@cairn/client";

import type { SortBy } from "./address.ts";

export type NodeRow = Schema<"NodeRow">;
export type RankTerms = Schema<"RankTerms">;
export type RankConstants = Schema<"RankConstants">;

/** One term of the blend: its signal, its normalized term, its constant, and what it adds. */
export interface RankPart {
  signal: "urgency" | "late" | "gravity" | "leverage";
  term: number;
  weight: number;
  adds: number;
}

/** Priority: the blend's parts that add to `terms.rank`, largest first; ties by the blend's order. */
export function rankParts(terms: RankTerms, constants: RankConstants): RankPart[] {
  const part = (signal: RankPart["signal"], term: number, weight: number): RankPart => ({ signal, term, weight, adds: term * weight });
  const parts = [
    part("urgency", terms.urgency, constants.urgency),
    part("late", terms.late, constants.late),
    part("gravity", terms.gravity_norm, constants.gravity),
    part("leverage", terms.leverage_norm, constants.leverage),
  ];
  return parts.filter((each) => each.adds > 0).sort((left, right) => right.adds - left.adds);
}

/**
 * A signal's value on a row as the next list shows it; none when the row has none (no
 * deadline; no estimate for effort, which is gravity per estimated day).
 */
export function signalOf(row: NodeRow, sort: SortBy, estimate?: number): number | string | undefined {
  switch (sort) {
    case "rank":
      return row.rank?.rank;
    case "slack":
      return row.slack_days ?? undefined;
    case "gravity":
      return row.gravity;
    case "leverage":
      return row.leverage;
    case "due":
      return row.due ?? undefined;
    case "effort":
      return estimate === undefined || estimate <= 0 ? undefined : row.gravity / estimate;
  }
}

/** A part in words: what the signal is on this row and what it adds to the rank. */
export function partWords(part: RankPart, row: NodeRow): string {
  const adds = `+${part.adds.toFixed(2)}`;
  switch (part.signal) {
    case "urgency":
      return `${startWords(row.slack_days ?? 0)} (${adds})`;
    case "late":
      return `${String(-(row.slack_days ?? 0))} days past its latest start (${adds})`;
    case "gravity":
      return `gravity ${row.gravity.toFixed(1)}, ${percent(part.term)} of the most in the journey (${adds})`;
    case "leverage":
      return `leverage ${row.leverage.toFixed(1)}, ${percent(part.term)} of the most (${adds})`;
  }
}

/** Urgency's reason: how soon its latest start is (F3: slack is latest start minus today). */
function startWords(slack: number): string {
  if (slack > 0) {
    return `latest start in ${String(slack)} days`;
  }
  return slack === 0 ? "latest start is today" : "its latest start has passed";
}

function percent(term: number): string {
  return `${String(Math.round(term * 100))}%`;
}

/** C10: why `row` ranks where it does, in one line, for the list sorted by `sort`. */
export function whyWords(row: NodeRow, constants: RankConstants, sort: SortBy, estimate?: number): string {
  const terms = row.rank;
  const blend =
    terms == null
      ? "not ranked"
      : rankParts(terms, constants)
          .map((part) => partWords(part, row))
          .join("; ") || "nothing adds to its rank yet: no deadline, no gravity or leverage beyond its own";
  const rank = terms == null ? "" : `rank ${terms.rank.toFixed(3)}: `;
  if (sort === "rank") {
    return `${rank}${blend}`;
  }
  const value = signalOf(row, sort, estimate);
  const shown = value === undefined ? (sort === "slack" || sort === "due" ? "no deadline" : "none") : typeof value === "number" ? value.toFixed(sort === "slack" ? 0 : 1) : value;
  return `sorted by ${sort}: ${shown}. Its ${rank}${blend}`;
}
