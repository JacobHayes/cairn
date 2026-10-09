// B6: what a snooze can set aside and until when (design 6.5): the node itself or an ancestor
// container holding unfinished work, each with its open count, nearest first; and the quick
// dates. Pure, so the unit tests check them without a page.
import { dateOf, dayOf } from "../timeline/model.ts";
import { isTerminal, nodeOf, recordOf, type NodeDetail, type Ready } from "./model.ts";

/** Something a snooze can set aside. */
export interface SetAside {
  node: string;
  title: string;
  /** For a container: the unfinished in-scope items beneath it (the node itself has none). */
  open: number | undefined;
}

/** Whether `key` lies beneath `ancestor`. */
function beneath(view: Ready, key: string, ancestor: string): boolean {
  let parent = nodeOf(view, key)?.parent ?? undefined;
  const seen = new Set<string>();
  while (parent !== undefined && !seen.has(parent)) {
    if (parent === ancestor) {
      return true;
    }
    seen.add(parent);
    parent = nodeOf(view, parent)?.parent ?? undefined;
  }
  return false;
}

/** The unfinished, in-scope work beneath a container: what its snooze would hold back. */
export function openUnder(view: Ready, container: string): number {
  return (view.journey.graph.nodes ?? []).filter((node) => {
    const derived = view.derived.nodes[node.key];
    return node.kind !== "group" && beneath(view, node.key, container) && derived?.relevance.value !== "not_relevant" && !isTerminal(recordOf(view, node).state);
  }).length;
}

/** The node, then each ancestor container with unfinished in-scope work, nearest first, with its open count. */
export function setAsideOptions(view: Ready, detail: NodeDetail): SetAside[] {
  const own: SetAside = { node: detail.node.key, title: detail.node.title, open: undefined };
  const ancestors = [...detail.ancestors].reverse().flatMap((ancestor): SetAside[] => {
    const open = openUnder(view, ancestor.key);
    return open === 0 ? [] : [{ node: ancestor.key, title: ancestor.title, open }];
  });
  return [own, ...ancestors];
}

/** The day after `today`. */
export function tomorrow(today: string): string {
  return dateOf(dayOf(today) + 1);
}

/** The next Monday after `today` (a Monday asks for the one a week on). */
export function nextMonday(today: string): string {
  const day = dayOf(today);
  // 1970-01-01 was a Thursday, so day 4 is the first Monday.
  const toMonday = (((4 - day) % 7) + 7) % 7;
  return dateOf(day + (toMonday === 0 ? 7 : toMonday));
}

/** Everything at or beneath `key`: the nodes a snooze of it must not wait on (B6). */
export function withinScope(view: Ready, key: string): Set<string> {
  return new Set([key, ...(view.journey.graph.nodes ?? []).filter((node) => beneath(view, node.key, key)).map((node) => node.key)]);
}
