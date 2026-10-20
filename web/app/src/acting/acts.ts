// C11's done with its inline evidence, D2's assign, and C9's bulk actions over a selection.
// (Which actions a node offers is detail/offers.ts's, for every surface.) Each is one patch: a bulk
// action is one patch with one mutation, so one event, per node (C9), and a guard failure on
// any node rejects it whole and names which (A15, D4).
import type { Schema } from "@cairn/client";

import { movesFrom, nodeOf, recordOf, storedSnooze, transition, type GraphNode, type Mutation, type NodeDerived, type Ready, type State } from "../detail/model.ts";

/** What the acting surfaces read about one node. */
export interface Facts {
  node: GraphNode;
  state: State;
  derived: NodeDerived;
  /** A stored snooze, whether or not it holds (B6). */
  snoozed: boolean;
}

/** Node `key`'s facts, or undefined when the journey does not hold it. */
export function factsOf(view: Ready, key: string): Facts | undefined {
  const node = nodeOf(view, key);
  const derived = view.derived.nodes[key];
  if (node === undefined || derived === undefined) {
    return undefined;
  }
  return { node, state: recordOf(view, node).state, derived, snoozed: storedSnooze(view, key) };
}

/** C9: the facts of every selected node the journey still holds, in the order selected, whatever page each is on. */
export function selectionOf(view: Ready, keys: Iterable<string>): Facts[] {
  return [...keys].flatMap((key): Facts[] => {
    const facts = factsOf(view, key);
    return facts === undefined ? [] : [facts];
  });
}

/** What done needs added first, as the engine derived it: an artifact link (G2) and a note (G4), each only when the node requires it and has none. */
export function missingEvidence(view: Ready, node: GraphNode): { artifact: boolean; note: boolean } {
  const guards = view.derived.nodes[node.key]?.failing_guards ?? [];
  return { artifact: guards.includes("has_artifact"), note: guards.includes("has_note") };
}

/** What done adds before completing: an artifact link and a note, each with its new key. */
export interface Evidence {
  artifact?: { key: string; url: string };
  note?: { key: string; text: string };
}

/** The done form's fields, as typed. */
export interface EvidenceDraft {
  artifact: string;
  note: string;
}

/** Whether the done form holds what the node needs, so sending it would complete the node (G2, G4). */
export function evidenceComplete(needs: { artifact: boolean; note: boolean }, draft: EvidenceDraft): boolean {
  return (!needs.artifact || draft.artifact.trim() !== "") && (!needs.note || draft.note.trim() !== "");
}

/** A saved done form: before notes it held the artifact address alone, as text. */
export function evidenceDraft(saved: EvidenceDraft | string): EvidenceDraft {
  return typeof saved === "string" ? { artifact: saved, note: "" } : saved;
}

/**
 * C11 done, with the inline artifact link (G2) and note (G4) a node that requires them and has
 * none needs: each added, then the completion, in one patch so the guards see them.
 */
export function doneMutations(key: string, evidence: Evidence = {}): Mutation[] {
  const { artifact, note } = evidence;
  return [
    ...(artifact === undefined ? [] : [{ op: "add_annotation" as const, annotation: { key: artifact.key, node: key, artifact: artifact.url } }]),
    ...(note === undefined ? [] : [{ op: "add_annotation" as const, annotation: { key: note.key, node: key, note: note.text } }]),
    transition(key, "complete"),
  ];
}

/** D2, E2: make `entity` the owner of `key`, explicitly. */
export function assignOwner(key: string, entity: string): Mutation {
  return { op: "set_participation", node: key, kind: "k_owner", source: [entity] };
}

/** C9: an action over every selected node. */
export type BulkAction =
  | { act: "start" }
  | { act: "done" }
  | { act: "skip"; reason: string }
  | { act: "assign"; entity: string }
  | { act: "snooze"; until: Schema<"SnoozeTarget"> }
  | { act: "unsnooze" };

/** The one patch's mutations, or the selected nodes the action cannot apply to. */
export type BulkPlan = { mutations: Mutation[] } | { unable: string[] };

/** The mutation `action` makes on one node, or undefined when its kind or state cannot take it. */
function bulkMutation(action: BulkAction, facts: Facts): Mutation | undefined {
  const { node, state } = facts;
  const moves = movesFrom(node.kind, state);
  const finished = moves.includes("reopen");
  switch (action.act) {
    case "start":
      return moves.includes("start") ? transition(node.key, "start") : undefined;
    case "done":
      return moves.includes("complete") ? transition(node.key, "complete") : moves.includes("reach") ? transition(node.key, "reach") : undefined;
    case "skip":
      return moves.includes("skip") ? transition(node.key, "skip", action.reason) : undefined;
    case "assign":
      return assignOwner(node.key, action.entity);
    case "snooze":
      return finished ? undefined : { op: "snooze", node: node.key, until: action.until };
    case "unsnooze":
      return facts.snoozed ? { op: "unsnooze", node: node.key } : undefined;
  }
}

const VERB: Record<BulkAction["act"], string> = {
  start: "start",
  done: "be marked done",
  skip: "be skipped",
  assign: "be assigned",
  snooze: "be snoozed",
  unsnooze: "be unsnoozed",
};

/**
 * Why `action` is not offered over `selected`: the members whose kind or state cannot take it,
 * named, or nothing when every one can. The actions are enabled only then, so none is sent that
 * is certain to be refused; the engine still checks the guards on the whole patch.
 */
export function reasonFor(action: BulkAction, selected: Facts[], title: (key: string) => string): string | undefined {
  const plan = bulkPlan(action, selected);
  if (!("unable" in plan)) {
    return undefined;
  }
  const [first = "", ...rest] = plan.unable.map(title);
  return `${first}${rest.length === 0 ? "" : ` and ${String(rest.length)} more`} cannot ${VERB[action.act]} in ${rest.length === 0 ? "its" : "their"} current state`;
}

/**
 * C9: `action` over `selected` as one patch, one mutation per node in the order given; or,
 * when any selected node's kind or state cannot take it, those nodes, so nothing is sent
 * partly. Guards (D4) are the engine's: a node that fails one rejects the whole patch.
 */
export function bulkPlan(action: BulkAction, selected: Facts[]): BulkPlan {
  const mutations: Mutation[] = [];
  const unable: string[] = [];
  for (const facts of selected) {
    const mutation = bulkMutation(action, facts);
    if (mutation === undefined) {
      unable.push(facts.node.key);
    } else {
      mutations.push(mutation);
    }
  }
  return unable.length > 0 ? { unable } : { mutations };
}

/** What a bulk action came to: refused before sending, naming the nodes, or sent, and whether it landed. */
export type BulkOutcome = { outcome: "unable"; unable: string[] } | { outcome: "sent"; landed: boolean };

/**
 * C9: `action` over `selected`, sent through `run` as one patch (one mutation, so one event,
 * per node), or refused before anything is sent when a node cannot take it. Selections act only
 * through this.
 */
export async function runBulk(action: BulkAction, selected: Facts[], run: (mutations: Mutation[]) => Promise<boolean>): Promise<BulkOutcome> {
  const plan = bulkPlan(action, selected);
  if ("unable" in plan) {
    return { outcome: "unable", unable: plan.unable };
  }
  return { outcome: "sent", landed: await run(plan.mutations) };
}
