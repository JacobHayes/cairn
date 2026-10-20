// Where a snooze ends, in words, for every surface that shows it (a card's foot, a list row, the
// stalled panel, a sentence, History): one date format and one phrase for a wait on a node.
import type { Schema } from "@cairn/client";

import { titleOf, type NodeDerived, type Ready } from "../detail/model.ts";
import { dateWords } from "./when.ts";

/** What follows a node's name where a snooze waits on it. */
export const SNOOZE_NODE_TAIL = " is done";

/** The date or node a snooze ends at. */
type Target = Schema<"SnoozeTarget">;

/**
 * Where a snooze ends: `Oct 20`, or `<node> is done` (`tail` is what a short room must not cut,
 * so the name takes the ellipsis). A removed node reads as a removed item, never its key.
 */
export function snoozeEnd(view: Ready, target: Target): { name: string; tail: string } {
  return "date" in target ? { name: dateWords(target.date, view.derived.today), tail: "" } : { name: titleOf(view, target.node), tail: SNOOZE_NODE_TAIL };
}

/** Where a snooze ends as one phrase: `Oct 20`, `Kickoff is done`. */
export function snoozeEndWords(view: Ready, target: Target): string {
  const { name, tail } = snoozeEnd(view, target);
  return `${name}${tail}`;
}

/**
 * Where a node's snooze ends, for a card's foot, a list row or a timeline row: `Until Oct 20`,
 * `Until <node> is done`, or `Via <container>` for a node held by its container's snooze. `title`
 * is the full sentence for a tooltip. None when the node is not snoozed.
 */
export function snoozeWords(view: Ready, derived: NodeDerived): { words: string; tail?: string; title: string } | undefined {
  if (derived.snoozed_via != null) {
    const via = titleOf(view, derived.snoozed_via);
    return { words: `Via ${via}`, title: `Snoozed through ${via}` };
  }
  const target = derived.snoozed;
  if (derived.display_state !== "snoozed" || target == null) {
    return undefined;
  }
  const { name, tail } = snoozeEnd(view, target);
  // The chip already says snoozed, so the words are where it ends, the one fact that must not be cut.
  return { words: `Until ${name}`, ...(tail === "" ? {} : { tail }), title: `Snoozed until ${name}${tail}` };
}
