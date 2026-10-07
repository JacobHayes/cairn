// C11: when the decision walkthrough has no decision to act on, it shows what would unblock
// the earliest ones: the open decisions still in scope, earliest start first, each with the
// milestones and other dependencies it waits on. A decision waits on its own unsatisfied
// dependencies (explicit requirements, condition gates, a stage opening: `blocked_by`), on
// those its ancestors hold for it (`blocked_through`, each listing its own), and on a snooze's
// target when one holds.
import type { Schema } from "@cairn/client";

import { isTerminal, nodeOf, recordOf, type GraphNode, type NodeKind, type Ready } from "../detail/model.ts";

/** What one waiting decision waits on. */
export interface Unblocker {
  node: string;
  kind: NodeKind;
  /** How the dependency arose (Containment, Gating), or `snooze` for a snooze's target. */
  via: Schema<"DependencyVia"> | "snooze";
  /** The ancestor whose own dependency it is, when the decision waits on it through containment. */
  through?: string;
}

/** An open decision the walkthrough cannot offer yet, and what would unblock it. */
export interface Waiting {
  decision: string;
  /** Its earliest start (F3), when the date network gives one. */
  earliest: string | undefined;
  unblockers: Unblocker[];
  /** A date snooze's date, when it is snoozed until a date. */
  snoozedUntil: string | undefined;
}

/** How many waiting decisions the walkthrough shows: the earliest few are what to unblock first. */
export const WAITING_SHOWN_MAX = 5;

function unblockersOf(view: Ready, decision: GraphNode): { unblockers: Unblocker[]; snoozedUntil: string | undefined } {
  const derived = view.derived.nodes[decision.key];
  const found = new Map<string, Unblocker>();
  const add = (node: string, via: Unblocker["via"], through?: string) => {
    const kind = nodeOf(view, node)?.kind;
    if (kind !== undefined && !found.has(node)) {
      found.set(node, through === undefined ? { node, kind, via } : { node, kind, via, through });
    }
  };
  // An ancestor's own list also names its unfinished children (containment), which hold the
  // ancestor, not the decision beneath it.
  const holders = [decision.key, ...(derived?.blocked_through ?? [])];
  for (const holder of holders) {
    for (const blocker of view.derived.nodes[holder]?.blocked_by ?? []) {
      if (blocker.via !== "containment") {
        add(blocker.node, blocker.via, holder === decision.key ? undefined : holder);
      }
    }
  }
  const snooze = derived?.snoozed;
  let snoozedUntil: string | undefined;
  if (snooze != null) {
    if ("node" in snooze) {
      add(snooze.node, "snooze");
    } else {
      snoozedUntil = snooze.date;
    }
  }
  return { unblockers: [...found.values()], snoozedUntil };
}

/** C11: the decisions on the acting frontier, whoever owns them, in rank order. */
export function actingDecisions(view: Ready): string[] {
  return view.derived.acting_frontier.filter((key) => nodeOf(view, key)?.kind === "decision");
}

/**
 * C11: the open, in-scope decisions off the acting frontier, earliest start first (none last),
 * then by key; each with what it waits on, milestones first.
 */
export function waitingDecisions(view: Ready): Waiting[] {
  const acting = new Set(view.derived.acting_frontier);
  const waiting: Waiting[] = [];
  for (const node of view.journey.graph.nodes ?? []) {
    const derived = view.derived.nodes[node.key];
    if (node.kind !== "decision" || derived === undefined || acting.has(node.key)) {
      continue;
    }
    const inScope = derived.relevance.value !== "not_relevant" && derived.effectively_skipped !== true;
    if (!inScope || isTerminal(recordOf(view, node).state)) {
      continue;
    }
    const { unblockers, snoozedUntil } = unblockersOf(view, node);
    unblockers.sort((left, right) => Number(right.kind === "milestone") - Number(left.kind === "milestone") || left.node.localeCompare(right.node));
    waiting.push({ decision: node.key, earliest: derived.dates.earliest_start?.date, unblockers, snoozedUntil });
  }
  return waiting.sort(
    (left, right) =>
      (left.earliest ?? "9999-12-31").localeCompare(right.earliest ?? "9999-12-31") || left.decision.localeCompare(right.decision),
  );
}
