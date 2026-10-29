// Proposal review's model (C14, B7, B8, B9, I6): what each conflict offers, the reviewer's
// edits to a proposal's items and mutations, the diff between the destination's graph before
// and after, and what stops an apply. Everything here is pure; the screen draws it and the
// host commits it. Strict validation stays the engine's, at preview and at apply.
import type { Schema } from "@cairn/client";

import { violationNode } from "../data/writes.ts";
import type { Edge, Graph, GraphNode, Mutation } from "../authoring/graph.ts";

export type Proposal = Schema<"Proposal">;
export type ProposalDraft = Schema<"ProposalDraft">;
export type ProposalPreview = Schema<"ProposalPreview">;
export type ReviewItem = Schema<"ReviewItem">;
export type Conflict = Schema<"Conflict">;
export type Resolution = Schema<"ConflictResolution">;
export type Mapping = Schema<"ParticipationMapping">;
export type UnresolvedReason = Schema<"UnresolvedReason">;

/** The kinds of resolution a conflict can offer (B7). */
export type ResolutionKind = Schema<"ResolutionKind">;
export type ConflictReview = Schema<"ConflictReview">;

/** Which kind of resolution `resolution` is. */
export function kindOf(resolution: Resolution): ResolutionKind {
  if (typeof resolution === "string") {
    return resolution;
  }
  if ("remap_role" in resolution) {
    return "remap_role";
  }
  return "remap_kind" in resolution ? "remap_kind" : "map_choices";
}

/** The node a conflict is about, when it is about one. */
export function conflictNode(conflict: Conflict): string | undefined {
  switch (conflict.about) {
    case "field":
    case "participation":
    case "resource":
      return conflict.node;
    case "shape":
      return conflict.journey.key;
    case "answer":
      return conflict.decision;
    case "edge":
      return conflict.edge.node;
    case "role":
    case "kind":
    case "default_owner":
      return undefined;
  }
}

/** The node a review item is about, when it is about one. */
export function itemNode(item: ReviewItem): string | undefined {
  switch (item.item) {
    case "conflict":
      return conflictNode(item.conflict);
    case "kept_local_edit":
      return typeof item.kept === "object" && "node" in item.kept ? item.kept.node.node : undefined;
    case "orphan":
    case "exclusion":
      return item.node;
    case "cascade":
      return item.removal.node;
    case "participation":
      return item.uses[0]?.node;
    case "violation":
      return violationNode(item.violation);
  }
}

function items(draft: ProposalDraft): ReviewItem[] {
  return draft.items ?? [];
}

function mutations(draft: ProposalDraft): Mutation[] {
  return draft.mutations ?? [];
}

function withItem(draft: ProposalDraft, index: number, change: (item: ReviewItem) => ReviewItem): ProposalDraft {
  return { ...draft, items: items(draft).map((item, at) => (at === index ? change(item) : item)) };
}

/** C14: the draft with conflict `index` resolved by `resolution`, or its choice cleared. */
export function withResolution(draft: ProposalDraft, index: number, resolution: Resolution | null): ProposalDraft {
  return withItem(draft, index, (item) => (item.item === "conflict" ? { ...item, resolution } : item));
}

/** B7: the draft with orphan `index` kept or removed with its descendants. */
export function withOrphanKept(draft: ProposalDraft, index: number, keep: boolean): ProposalDraft {
  return withItem(draft, index, (item) => (item.item === "orphan" ? { ...item, keep } : item));
}

/** B8: the draft with identity `index`'s participations mapped, or the mapping cleared. */
export function withMapping(draft: ProposalDraft, index: number, mapping: Mapping | null): ProposalDraft {
  return withItem(draft, index, (item) => (item.item === "participation" ? { ...item, mapping } : item));
}

/** B8: the draft with node `index` left out of the saved route, or put back. */
export function withExclusion(draft: ProposalDraft, index: number, excluded: boolean): ProposalDraft {
  return withItem(draft, index, (item) => (item.item === "exclusion" ? { ...item, excluded } : item));
}

/** C14: the draft without its mutation at `index`. */
export function withoutMutation(draft: ProposalDraft, index: number): ProposalDraft {
  return { ...draft, mutations: mutations(draft).filter((_, at) => at !== index) };
}

/** C14: the draft with its mutation at `index` replaced. */
export function withMutation(draft: ProposalDraft, index: number, mutation: Mutation): ProposalDraft {
  return { ...draft, mutations: mutations(draft).map((each, at) => (at === index ? mutation : each)) };
}

/** C14: the draft with `added` after its mutations. */
export function withMutationsAdded(draft: ProposalDraft, added: Mutation[]): ProposalDraft {
  return { ...draft, mutations: [...mutations(draft), ...added] };
}

/** What one node changed between two graphs: the fields that differ. */
export type Changed = Record<string, string[]>;

/** C14: what a proposal does to its destination's graph, node by node and edge by edge. */
export interface GraphDiff {
  added: string[];
  removed: string[];
  changed: Changed;
  edgesAdded: Edge[];
  edgesRemoved: Edge[];
}

function byKey(graph: Graph | null | undefined): Map<string, GraphNode> {
  return new Map((graph?.nodes ?? []).map((node) => [node.key, node]));
}

function edgesOf(graph: Graph | null | undefined): Edge[] {
  return (graph?.nodes ?? []).flatMap((node) => (node.requires ?? []).map((requires) => ({ node: node.key, requires })));
}

const edgeName = (edge: Edge) => `${edge.node}>${edge.requires}`;

/** The node fields whose values differ, by their JSON, in name order. */
function fieldsDiffering(before: GraphNode, after: GraphNode): string[] {
  const fields = new Set([...Object.keys(before), ...Object.keys(after)]);
  const value = (node: GraphNode, field: string) => JSON.stringify((node as Record<string, unknown>)[field] ?? null);
  return [...fields].filter((field) => field !== "key" && value(before, field) !== value(after, field)).sort();
}

/** The journey state kept per node, which a proposal's pins, answers, transitions and snoozes change without touching the node's definition. */
const STATE_FIELDS = ["nodes", "answers", "rationales", "overrides", "pins", "snoozes", "ticks"] as const;

/** What a journey's graph holds of node `key` in its state, as one comparable text; none for a graph with no state. */
export function stateOf(graph: Graph | null | undefined, key: string): Record<string, unknown> {
  const state = graph?.state as Record<string, Record<string, unknown> | undefined> | undefined;
  return Object.fromEntries(STATE_FIELDS.flatMap((field) => (state?.[field]?.[key] === undefined ? [] : [[field, state[field][key]]])));
}

/** C14: the diff from `before` to `after`, each list in key order. A node whose journey state differs (not its definition) lists the field `state`. */
export function graphDiff(before: Graph | null | undefined, after: Graph | null | undefined): GraphDiff {
  const [was, now] = [byKey(before), byKey(after)];
  const added = [...now.keys()].filter((key) => !was.has(key)).sort();
  const removed = [...was.keys()].filter((key) => !now.has(key)).sort();
  const changed: Changed = {};
  for (const [key, node] of now) {
    const old = was.get(key);
    const fields = old === undefined ? [] : fieldsDiffering(old, node);
    if (old !== undefined && JSON.stringify(stateOf(before, key)) !== JSON.stringify(stateOf(after, key))) {
      fields.push("state");
    }
    if (fields.length > 0) {
      changed[key] = fields;
    }
  }
  const [edgesBefore, edgesAfter] = [edgesOf(before), edgesOf(after)];
  const namesBefore = new Set(edgesBefore.map(edgeName));
  const namesAfter = new Set(edgesAfter.map(edgeName));
  const order = (left: Edge, right: Edge) => edgeName(left).localeCompare(edgeName(right));
  return {
    added,
    removed,
    changed,
    edgesAdded: edgesAfter.filter((edge) => !namesBefore.has(edgeName(edge))).sort(order),
    edgesRemoved: edgesBefore.filter((edge) => !namesAfter.has(edgeName(edge))).sort(order),
  };
}

/** The editor of the node being typed in the additions: the one hold of an editor that is not a problem to fix. */
export const ADD_NODE_EDITOR = "add-node";

/** I6: why a proposal cannot be applied now, each a reason the screen names. */
export type Blocker = "not_open" | "editing" | "unsaved" | "stale" | "unresolved" | "invalid" | "no_preview";

/** What the apply gate reads. */
export interface GateInput {
  proposal: Pick<Proposal, "status" | "revision">;
  /** The reviewer has edits the proposal does not hold yet. */
  dirty: boolean;
  /** An editor holds a value with a problem, which the proposal cannot take until it is fixed. */
  editing: boolean;
  /** I6: its destination moved since it was drafted. */
  stale: boolean;
  /** The preview of the proposal as held, or none yet. */
  preview: Pick<ProposalPreview, "unresolved" | "violations"> | undefined;
}

/**
 * C14, I6: what stops applying the proposal now, in the order to fix them: it must be open,
 * have no editor holding a value with a problem, hold every edit, stand on its destination as it is (a stale one is refreshed first), have a
 * choice for every item and no violation. None means it may be applied (the revision the reviewer
 * saw is the one sent, layer.ts `applyRevision`).
 */
export function applyBlockers(input: GateInput): Blocker[] {
  const blockers: Blocker[] = [];
  if (input.proposal.status !== "open") {
    return ["not_open"];
  }
  if (input.editing) {
    blockers.push("editing");
  }
  if (input.dirty) {
    blockers.push("unsaved");
  }
  if (input.stale) {
    blockers.push("stale");
  }
  if (input.preview === undefined) {
    blockers.push("no_preview");
  } else {
    if ((input.preview.unresolved ?? []).length > 0) {
      blockers.push("unresolved");
    }
    if ((input.preview.violations ?? []).length > 0) {
      blockers.push("invalid");
    }
  }
  return blockers;
}

/** I6: the patches that moved the destination from `expected` to `current`: the last ones of its history. */
export function intervening<P>(patches: readonly P[], expected: number, current: number): P[] {
  const count = Math.max(0, current - expected);
  return count === 0 ? [] : patches.slice(-count);
}

/** The kinds of change a proposal makes to a node. */
export type DiffKind = "added" | "changed" | "removed";

/** C14: how the layer marks one node a proposal touches: the kind of change, whether it needs a choice, and what more there is to say of it. */
export interface DiffMark {
  kind: DiffKind;
  /** An unresolved choice is about it: it counts under Needs a choice. */
  choice?: true;
  /** What else holds of it (a conflict, an orphan), for the hover and the list's foot. */
  note?: string;
}

/** The words of each kind of change, so the canvas, the list, the timeline and the rail say the same. */
export const DIFF_LABELS: Record<DiffKind, string> = {
  added: "Added",
  changed: "Changed",
  removed: "Removed",
};

/** What a mark adds to its kind of change. */
export const DIFF_NOTES = {
  conflict: "Needs a choice",
  orphan: "Orphaned, kept",
  orphanRemoved: "Orphaned, removed",
  excluded: "Left out",
} as const;

/**
 * C14: the layer's mark for each node the proposal touches: added, removed, or changed by the
 * diff or by a state mutation the graph does not show (`stateful`: a pin, an answer, a
 * transition); an orphan or a node left out of a saved route, which the items say more of than
 * the diff; and, when an unresolved conflict (`open`, the indexes of the items still needing a
 * choice) is about it, a choice besides its kind. A resolved conflict leaves only its kind.
 */
export function diffMarks(diff: GraphDiff, reviewItems: readonly ReviewItem[], open: ReadonlySet<number>, stateful: readonly string[] = []): Record<string, DiffMark> {
  const marks: Record<string, DiffMark> = {};
  for (const key of stateful) {
    marks[key] = { kind: "changed" };
  }
  for (const key of diff.added) {
    marks[key] = { kind: "added" };
  }
  for (const key of Object.keys(diff.changed)) {
    marks[key] = { kind: "changed" };
  }
  for (const key of diff.removed) {
    marks[key] = { kind: "removed" };
  }
  reviewItems.forEach((item, index) => {
    if (item.item === "orphan") {
      marks[item.node] = item.keep ? { kind: "changed", note: DIFF_NOTES.orphan } : { kind: "removed", note: DIFF_NOTES.orphanRemoved };
    } else if (item.item === "exclusion" && item.excluded) {
      marks[item.node] = { kind: "removed", note: DIFF_NOTES.excluded };
    } else if (item.item === "conflict" && open.has(index)) {
      const node = conflictNode(item.conflict);
      if (node !== undefined) {
        marks[node] = { kind: marks[node]?.kind ?? "changed", choice: true, note: DIFF_NOTES.conflict };
      }
    }
  });
  return marks;
}

/** One node the review lists: its mark, the fields that differ, and the removed container it goes with. */
export interface ReviewEntry {
  key: string;
  mark: DiffMark;
  /** The fields of a node that exists both before and after, that differ. */
  fields: string[];
  /** A removal cascaded from this removed ancestor: it is listed beneath it. */
  cause: string | undefined;
}

/**
 * C14: the marked nodes as the list shows them, in the order a reviewer reads them (adds, changes, and
 * removals), each removal that cascades from a removed container listed
 * under that container and not beside it.
 */
export function reviewEntries(diff: GraphDiff, marks: Record<string, DiffMark>, before: Graph | null | undefined): ReviewEntry[] {
  const parents = new Map([...byKey(before)].map(([key, node]) => [key, node.parent ?? undefined]));
  // The outermost removed ancestor: a whole removed subtree is listed under its root.
  const causeOf = (key: string): string | undefined => {
    let cause: string | undefined;
    for (let at = parents.get(key), steps = 0; at !== undefined && steps < parents.size; at = parents.get(at), steps += 1) {
      cause = marks[at]?.kind === "removed" ? at : cause;
    }
    return cause;
  };
  const entries = Object.entries(marks).map(([key, mark]): ReviewEntry => ({ key, mark, fields: diff.changed[key] ?? [], cause: mark.kind === "removed" ? causeOf(key) : undefined }));
  const order: DiffKind[] = ["added", "changed", "removed"];
  const roots = entries.filter((entry) => entry.cause === undefined).sort((left, right) => order.indexOf(left.mark.kind) - order.indexOf(right.mark.kind) || left.key.localeCompare(right.key));
  return roots.flatMap((root) => [root, ...entries.filter((entry) => entry.cause === root.key).sort((left, right) => left.key.localeCompare(right.key))]);
}

/**
 * C14: names for `next`'s mutations that stay with each mutation as the list changes, so an
 * editor stays with its change: carried over unchanged when mutations are edited in place,
 * the dropped one's name dropped when one is removed, fresh names for mutations added after
 * the rest; any other change (the proposal saved or refreshed elsewhere) names every one
 * afresh, and each editor starts again from what it holds.
 */
export function carryIds(previous: { mutations: readonly Mutation[]; ids: readonly string[] }, next: readonly Mutation[], fresh: () => string): string[] {
  const same = (left: Mutation | undefined, right: Mutation | undefined) => JSON.stringify(left) === JSON.stringify(right);
  const before = previous.mutations;
  if (before.length > 0 && next.length === before.length) {
    return [...previous.ids];
  }
  if (next.length === before.length - 1) {
    const dropped = next.findIndex((mutation, at) => !same(mutation, before[at]));
    const at = dropped === -1 ? before.length - 1 : dropped;
    if (next.slice(at).every((mutation, offset) => same(mutation, before[at + offset + 1]))) {
      return previous.ids.filter((_, index) => index !== at);
    }
  }
  if (next.length > before.length && before.every((mutation, at) => same(mutation, next[at]))) {
    return [...previous.ids, ...next.slice(before.length).map(() => fresh())];
  }
  return next.map(() => fresh());
}

/** The node with a field value written: a field cleared (null) is left out, as a node without it. */
export function withFieldValue(node: GraphNode, value: Record<string, unknown>): GraphNode {
  const cleared = new Set(Object.entries(value).flatMap(([field, set]) => (set === null ? [field] : [])));
  const kept = Object.entries({ ...node, ...value }).filter(([field]) => !cleared.has(field));
  return Object.fromEntries(kept) as GraphNode;
}

/** C14: whether the reviewer may owe `item` a choice (a conflict or an identity's mapping), so that while the edits are newer than the preview it shows as being checked rather than settled. */
export function needsChoice(item: ReviewItem): boolean {
  return item.item === "conflict" || item.item === "participation";
}
