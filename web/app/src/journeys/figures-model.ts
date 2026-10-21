// What a list of journeys says of each one, as data (4.8, C16): progress from the status summary,
// what needs a look (worst first), what is the viewer's, the next milestone, and the one fact a
// phone row leads with. Words come from the functions here, so the index, Home and the journey
// card say a thing one way. Pure, so the unit tests check them without a page.
import { STATUS_HOLDS_CURRENT_WORK, type Schema } from "@cairn/client";

import type { FactTone } from "../acting/reasons.ts";
import { openForViewer } from "../acting/yours.ts";
import type { Ready } from "../detail/model.ts";
import { dateInk, whenWords } from "../status/when.ts";
import { TROUBLES } from "../status/words.ts";
import type { SummaryModel } from "../summary/model.ts";
import type { JourneyStatus } from "./address.ts";
import type { MineEntry } from "./mine.ts";

/** Done over in scope: what is in scope less what has work left on it. */
export function progressOf(model: SummaryModel): { done: number; inScope: number } {
  return { done: model.inScope - model.remaining, inScope: model.inScope };
}

/**
 * What is still the viewer's to do (2.4): of their nodes, those still open for them (`openForViewer`),
 * and how many of those are on the acting frontier.
 */
export function yoursOf(ready: Ready, entries: readonly MineEntry[]): { open: number; actionable: number } {
  const open = entries.filter((entry) => openForViewer(ready, entry.node));
  const frontier = new Set(ready.derived.acting_frontier);
  return { open: open.length, actionable: open.filter((entry) => frontier.has(entry.node)).length };
}

/** A kind of trouble a journey can be in; all but `stalled` are list flags of the same name. */
export type TroubleKind = "overdue" | "shortfall" | "stale" | "stalled";

/** One kind of trouble: how many nodes it touches, and the ink it is said in. */
export interface Trouble {
  kind: TroubleKind;
  count: number;
  tone: FactTone | undefined;
}

/**
 * What needs a look in a journey, worst first: overdue and short of days (error), stale (warning),
 * then that nothing can move yet (untoned: it is often an ordinary wait). Only what is not zero.
 */
export function troubleOf(model: SummaryModel, stalled: boolean): Trouble[] {
  const found: Trouble[] = [...TROUBLES.map((each) => ({ kind: each.flag, count: each.count(model), tone: each.tone })), { kind: "stalled", count: stalled ? 1 : 0, tone: undefined }];
  return found.filter((each) => each.count > 0);
}

/** A trouble in words, as the one table says it: `2 nodes overdue`, `Nothing can move yet`. */
export function troubleWords(trouble: Trouble): string {
  const each = TROUBLES.find((found) => found.flag === trouble.kind);
  return each === undefined ? "Nothing can move yet" : each.words(trouble.count);
}

/** What is the viewer's in a journey, for the cell that says so. */
export type ForYou =
  | { kind: "ready"; count: number }
  /** Open work of theirs, none of it able to move yet. */
  | { kind: "waiting" }
  /** They hold things here, none of them still open. */
  | { kind: "nothing-open" }
  | { kind: "nothing" }
  /** The journey is completed or archived: it holds no current work (B11). */
  | { kind: "inactive" };

/** The viewer's part in a journey of `status` that they hold `mine` of, `ready` of which can move now. */
export function forYouOf(view: Ready, status: JourneyStatus, mine: readonly MineEntry[], ready: number): ForYou {
  if (!STATUS_HOLDS_CURRENT_WORK[status]) {
    return { kind: "inactive" };
  }
  if (ready > 0) {
    return { kind: "ready", count: ready };
  }
  if (mine.length === 0) {
    return { kind: "nothing" };
  }
  return mine.some((entry) => openForViewer(view, entry.node)) ? { kind: "waiting" } : { kind: "nothing-open" };
}

/** What a viewer's part is, in words. */
export function forYouWords(forYou: ForYou): string {
  switch (forYou.kind) {
    case "ready":
      return `${String(forYou.count)} ready`;
    case "waiting":
      return "Waiting";
    case "nothing-open":
      return "Nothing open";
    case "nothing":
      return "Nothing yours";
    case "inactive":
      return "Not active";
  }
}

/** The next milestone as the table says it: its title, its date in words (how far off, or how late), and the ink for a soon or late date. */
export function milestoneOf(model: SummaryModel, today: string): { title: string; when: string; tone: FactTone | undefined } | undefined {
  const next = model.upcoming[0];
  if (next === undefined) {
    return undefined;
  }
  return { title: next.title, when: whenWords(next.date, next.origin, today), tone: dateInk(next.date, today) };
}

/** C16: that a journey's route has a newer version, from the host's field and never worked out from versions; none when it has not. */
export function upgradeWords(summary: Schema<"JourneySummary">): string | undefined {
  if (!summary.upgrade_available) {
    return undefined;
  }
  return summary.latest_version == null ? "Upgrade available" : `Version ${String(summary.latest_version)} available`;
}
