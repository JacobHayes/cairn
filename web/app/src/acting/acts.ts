// C11's per-kind actions, shared by the triage card and the next list's rows (C10 offers the
// same actions inline), and C9's bulk actions over a selection. Each is one patch: a bulk
// action is one patch with one mutation, so one event, per node (C9), and a guard failure on
// any node rejects it whole and names which (A15, D4).
import type { Schema } from "@cairn/client";

import { movesFrom, nodeOf, recordOf, transition, type GraphNode, type Mutation, type NodeDerived, type Ready, type State } from "../detail/model.ts";

/** One action a card or row offers. */
export type Act = "answer" | "start" | "done" | "reach" | "skip" | "snooze" | "breakdown" | "atomic" | "canvas";

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
  const snoozed = view.journey.graph.state?.snoozes?.[key] !== undefined;
  return { node, state: recordOf(view, node).state, derived, snoozed };
}

/** C9: the facts of every selected node the journey still holds, in the order selected, whatever page each is on. */
export function selectionOf(view: Ready, keys: Iterable<string>): Facts[] {
  return [...keys].flatMap((key): Facts[] => {
    const facts = factsOf(view, key);
    return facts === undefined ? [] : [facts];
  });
}

/**
 * C11: the actions a card offers for its node's kind, in the order offered. A placeholder that
 * needs breaking down offers break down (which opens a proposal), mark atomic, and snooze, and
 * no done (B10); once it has children or is atomic it reverts to its kind's actions.
 * Pass and assign owner (D2) are every card's, not the kind's.
 */
export function actsFor({ node, state, derived }: Facts): Act[] {
  if (derived.needs_breakdown === true) {
    return ["breakdown", "atomic", "snooze"];
  }
  const moves = movesFrom(node.kind, state);
  switch (node.kind) {
    case "decision":
      return ["answer", "skip", "snooze"];
    case "deliverable":
    case "action":
      return [...(moves.includes("start") ? (["start"] as const) : []), "done", "snooze", "skip", "canvas"];
    case "milestone":
      return ["reach", "skip", "snooze", "canvas"];
    case "group":
      return [];
  }
}

/** G2: whether a node has a link designated as its artifact. */
export function hasArtifact(view: Ready, key: string): boolean {
  return (view.journey.graph.state?.annotations ?? []).some((annotation) => annotation.body.node === key && "artifact" in annotation.body);
}

/** G4: whether a node has a note of its own: not a link, and not one on a child or the journey. */
export function hasNote(view: Ready, key: string): boolean {
  return (view.journey.graph.state?.annotations ?? []).some((annotation) => annotation.body.node === key && "note" in annotation.body);
}

/** What done needs added first: an artifact link (G2) and a note (G4), each only when the node requires it and has none. */
export function missingEvidence(view: Ready, node: GraphNode): { artifact: boolean; note: boolean } {
  return {
    artifact: node.requires_artifact === true && !hasArtifact(view, node.key),
    note: node.requires_note === true && !hasNote(view, node.key),
  };
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
      return finished || node.kind === "group" ? undefined : { op: "snooze", node: node.key, until: action.until };
    case "unsnooze":
      return facts.snoozed ? { op: "unsnooze", node: node.key } : undefined;
  }
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
 * per node), or refused before anything is sent when a node cannot take it. The list's bulk
 * bar acts only through this.
 */
export async function runBulk(action: BulkAction, selected: Facts[], run: (mutations: Mutation[]) => Promise<boolean>): Promise<BulkOutcome> {
  const plan = bulkPlan(action, selected);
  if ("unable" in plan) {
    return { outcome: "unable", unable: plan.unable };
  }
  return { outcome: "sent", landed: await run(plan.mutations) };
}
