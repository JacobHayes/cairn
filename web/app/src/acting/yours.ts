// C10, C16: when the viewer holds work in a journey but none of it is on the acting frontier,
// the one line that answers them: what is theirs next, and what it waits on. The unranked
// "mine" projection says what they hold (E4); the date network says which starts first.
import type { MineEntry } from "../journeys/mine.ts";
import { milestonesFirst, nodeOf, titleOf, type Ready } from "../detail/model.ts";
import { snoozeEndWords } from "../status/snooze.ts";
import { isOpen } from "../status/state.ts";
import { unblockersOf } from "./waiting.ts";

/**
 * Whether node `key` is still the viewer's to do if it is in their "mine": in scope, not a
 * container, not skipped, not a milestone that reaches itself, and not finished (2.4). The one
 * rule Home's counts and the journey card's use, and the line for work that is all waiting.
 */
export function openForViewer(view: Ready, key: string): boolean {
  const node = nodeOf(view, key);
  const derived = view.derived.nodes[key];
  return (
    node !== undefined &&
    derived !== undefined &&
    node.kind !== "group" &&
    isOpen(derived)
  );
}

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
    return node !== undefined && derived !== undefined && openForViewer(view, entry.node) ? [{ node, start: derived.dates.earliest_start?.date ?? "9999-12-31" }] : [];
  });
  const first = open.sort((left, right) => left.start.localeCompare(right.start) || left.node.key.localeCompare(right.node.key))[0];
  if (first === undefined) {
    return undefined;
  }
  const { unblockers, snoozedUntil } = unblockersOf(view, first.node);
  // A container waits on its unfinished children, which `unblockersOf` leaves out for decisions.
  const children = (view.derived.nodes[first.node.key]?.blocked_by ?? []).filter((each) => each.via === "containment").map((each) => each.node);
  const after = [...new Set([...milestonesFirst(view, unblockers, (each) => each.node).map((each) => each.node), ...children])].map((key) => titleOf(view, key));
  return { node: first.node.key, after, snoozedUntil };
}

/** "A", "A and B", "A, B and 2 more". */
function named(titles: readonly string[]): string {
  const shown = titles.slice(0, NAMED_MAX);
  const more = titles.length - shown.length;
  return more > 0 ? `${shown.join(", ")} and ${String(more)} more` : shown.join(" and ");
}

/** What the next item waits on, in words: `after A and B`, or `snoozed until Oct 20`; empty when it names nothing. */
export function waitsOnWords(view: Ready, next: NextForYou): string {
  if (next.after.length > 0) {
    return `after ${named(next.after)}`;
  }
  return next.snoozedUntil === undefined ? "" : `snoozed until ${snoozeEndWords(view, { date: next.snoozedUntil })}`;
}

/** The line, in words. */
export function nextForYouWords(view: Ready, next: NextForYou): string {
  const waits = waitsOnWords(view, next);
  return `Next for you: ${titleOf(view, next.node)}${waits === "" ? "" : `, ${waits}`}.`;
}
