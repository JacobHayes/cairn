// C16: what Home shows, as data. The page reads each candidate journey (reading.ts) and this says
// which of the four states it is in, what the line under the title counts, and what of the
// viewer's work in a journey is held back. Pure, so the unit tests check the rules without a page.
import type { NodeRow } from "../acting/why.ts";
import { openForViewer } from "../acting/yours.ts";
import type { Ready } from "../detail/model.ts";
import type { MineEntry } from "./mine.ts";

/** What Home is in: still reading, unable to read, or one of the four states a read page can be in (C16). */
export type HomeState =
  /** The caller or the index has not answered, or no journey has settled with anything to show. */
  | "loading"
  | "failed"
  /** They are someone, and hold nothing in any active journey. */
  | "unassigned"
  /** They hold work, none of it able to move now. */
  | "quiet"
  /** Some of their work can move now, or a journey that may hold some could not be read. */
  | "ready";

/** One candidate journey as the state sees it. */
export interface HomeJourney {
  reading: "loading" | "failed" | "gone" | "ready";
  /** They hold something in it, finished or not. */
  participates: boolean;
  /** How many of their rows can move now. */
  ready: number;
}

/**
 * C16: the state Home is in. A journey that failed is unknown, so it keeps the page from saying
 * the viewer has nothing; one still being read keeps it from saying anything until something is
 * ready to show.
 */
export function homeState(input: {
  caller: "loading" | "failed" | "ready";
  index: "loading" | "failed" | "ready";
  journeys: readonly HomeJourney[];
  /** The proposals waiting on the viewer: Home does not say nothing needs them before they are read, or while there are some (C14, C16). */
  proposals: "loading" | "failed" | { waiting: number };
}): HomeState {
  if (input.caller === "failed" || input.index === "failed") {
    return "failed";
  }
  if (input.caller === "loading" || input.index === "loading") {
    return "loading";
  }
  if (input.journeys.some((each) => each.ready > 0 || each.reading === "failed")) {
    return "ready";
  }
  if (input.journeys.some((each) => each.reading === "loading")) {
    return "loading";
  }
  if (input.proposals === "loading" || input.proposals === "failed") {
    return input.proposals;
  }
  if (input.proposals.waiting > 0) {
    return "ready";
  }
  return input.journeys.some((each) => each.participates) ? "quiet" : "unassigned";
}

/** The viewer's work in a journey that is not ready: how much is still open, and how much of it is late. */
export function heldBack(view: Ready, mine: readonly MineEntry[], ready: readonly NodeRow[]): { count: number; late: number } {
  const movable = new Set(ready.map((row) => row.key));
  const held = mine.filter((entry) => !movable.has(entry.node) && openForViewer(view, entry.node));
  return { count: held.length, late: held.filter((entry) => view.derived.nodes[entry.node]?.overdue === true).length };
}

/** The line under Home's title once every journey is read: `6 ready for you in 3 journeys`; none while any is still to read or could not be. */
export function readyLine(journeys: readonly HomeJourney[]): { rows: number; journeys: number } | undefined {
  if (journeys.some((each) => each.reading === "loading" || each.reading === "failed")) {
    return undefined;
  }
  const withRows = journeys.filter((each) => each.ready > 0);
  return withRows.length === 0 ? undefined : { rows: withRows.reduce((sum, each) => sum + each.ready, 0), journeys: withRows.length };
}
