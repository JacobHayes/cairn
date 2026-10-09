// C10, C16: when the viewer holds work in a journey but none of it is on the acting frontier,
// the one line that answers them: what is theirs next, and what it waits on. The unranked
// "mine" projection says what they hold (E4); the date network says which starts first.
import type { MineEntry } from "../journeys/mine.ts";
import { isTerminal, nodeOf, recordOf, titleOf, type Ready } from "../detail/model.ts";
import { dateWords } from "../timeline/model.ts";
import { unblockersOf } from "./waiting.ts";

/** How many things the line names it waits on before it says "and N more". */
const NAMED_MAX = 2;

/** The viewer's earliest-starting open node, none of which is actionable now. */
export interface NextForYou {
  node: string;
  /** What it waits on, by title, the milestones first. */
  after: string[];
  /** The date a snooze holds it until, when one does and nothing else is named. */
  snoozedUntil: string | undefined;
}

/** The viewer's next item when nothing of theirs is on the acting frontier; none when something is, or nothing open is theirs. */
export function nextForYou(view: Ready, entries: readonly MineEntry[]): NextForYou | undefined {
  const frontier = new Set(view.derived.acting_frontier);
  if (entries.some((entry) => frontier.has(entry.node))) {
    return undefined;
  }
  const open = entries.flatMap((entry) => {
    const node = nodeOf(view, entry.node);
    const derived = view.derived.nodes[entry.node];
    const live = node !== undefined && derived !== undefined && node.kind !== "group" && derived.relevance.value !== "not_relevant" && derived.effectively_skipped !== true && derived.display_state !== "done" && !isTerminal(recordOf(view, node).state);
    return live ? [{ node, start: derived.dates.earliest_start?.date ?? "9999-12-31" }] : [];
  });
  const first = open.sort((left, right) => left.start.localeCompare(right.start) || left.node.key.localeCompare(right.node.key))[0];
  if (first === undefined) {
    return undefined;
  }
  const { unblockers, snoozedUntil } = unblockersOf(view, first.node);
  const milestonesFirst = [...unblockers].sort((left, right) => Number(right.kind === "milestone") - Number(left.kind === "milestone"));
  // A container waits on its unfinished children, which `unblockersOf` leaves out for decisions.
  const children = (view.derived.nodes[first.node.key]?.blocked_by ?? []).filter((each) => each.via === "containment").map((each) => each.node);
  const after = [...new Set([...milestonesFirst.map((each) => each.node), ...children])].map((key) => titleOf(view, key));
  return { node: first.node.key, after, snoozedUntil };
}

/** "A", "A and B", "A, B and 2 more". */
function named(titles: readonly string[]): string {
  const shown = titles.slice(0, NAMED_MAX);
  const more = titles.length - shown.length;
  return more > 0 ? `${shown.join(", ")} and ${String(more)} more` : shown.join(" and ");
}

/** The line, in words. */
export function nextForYouWords(view: Ready, next: NextForYou): string {
  const title = titleOf(view, next.node);
  if (next.after.length > 0) {
    return `Next for you: ${title}, after ${named(next.after)}.`;
  }
  return next.snoozedUntil === undefined ? `Next for you: ${title}.` : `Next for you: ${title}, snoozed until ${dateWords(next.snoozedUntil, view.derived.today)}.`;
}
