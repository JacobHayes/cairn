// C11: when the decision walkthrough has no decision to act on, it shows what would unblock
// the earliest ones: the open decisions still in scope, earliest start first, each with the
// milestones and other dependencies it waits on. A decision waits on what the engine derived it
// waits on (`waits_on`: its own unsatisfied dependencies and those its ancestors hold for it)
// and on a snooze's target when one holds.
import type { Schema } from "@cairn/client";

import { isStage, milestonesFirst, nodeOf, waitsOn, type GraphNode, type NodeKind, type Ready } from "../detail/model.ts";

/** What one waiting decision waits on. */
export interface Unblocker {
  node: string;
  kind: NodeKind;
  /** Whether it is a stage (a top-level group). */
  stage: boolean;
  /** How the dependency arose (Containment, Gating), or `snooze` for a snooze's target. */
  via: Schema<"DependencyVia"> | "snooze";
  /** The ancestor whose own dependency it is, when the decision waits on it through containment. */
  through?: string;
  /** The date it is scheduled for, while a date ahead holds it (D8). */
  date?: string;
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

/** What `decision` (or any node) waits on: its unblockers, milestones first by the caller, and the date a snooze waits for. */
export function unblockersOf(view: Ready, decision: GraphNode): { unblockers: Unblocker[]; snoozedUntil: string | undefined } {
  const derived = view.derived.nodes[decision.key];
  const found = new Map<string, Unblocker>();
  const add = (node: string, via: Unblocker["via"], through?: string, date?: string) => {
    const held = nodeOf(view, node);
    if (held !== undefined && !found.has(node)) {
      const base = { node, kind: held.kind, stage: isStage(held), via };
      found.set(node, { ...base, ...(through === undefined ? {} : { through }), ...(date === undefined ? {} : { date }) });
    }
  };
  for (const held of waitsOn(view, decision.key)) {
    add(held.node, held.via, held.holder, held.date);
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
 * C11: the summary's open, in-scope decisions that are off the acting frontier, earliest start
 * first (none last), then by key; each with what it waits on, milestones first.
 */
export function waitingDecisions(view: Ready, summary: Schema<"StatusSummary">): Waiting[] {
  const acting = new Set(view.derived.acting_frontier);
  const waiting: Waiting[] = [];
  for (const { node: key } of summary.open_decisions ?? []) {
    const node = nodeOf(view, key);
    const derived = view.derived.nodes[key];
    if (node === undefined || derived === undefined || acting.has(key)) {
      continue;
    }
    const { unblockers, snoozedUntil } = unblockersOf(view, node);
    waiting.push({ decision: key, earliest: derived.dates.earliest_start?.date, unblockers: milestonesFirst(view, unblockers, (each) => each.node), snoozedUntil });
  }
  return waiting.sort(
    (left, right) =>
      (left.earliest ?? "9999-12-31").localeCompare(right.earliest ?? "9999-12-31") || left.decision.localeCompare(right.decision),
  );
}
