// Proposal review's model (C14, B7, B8, B9, I6): what each conflict offers, the reviewer's
// edits to a proposal's items and mutations, the diff between the destination's graph before
// and after, and what stops an apply. Everything here is pure; the screen draws it and the
// host commits it. Strict validation stays the engine's, at preview and at apply.
import type { Schema } from "@cairn/client";

import type { Edge, Graph, GraphNode, Mutation } from "../authoring/graph.ts";

export type Proposal = Schema<"Proposal">;
export type ProposalDraft = Schema<"ProposalDraft">;
export type ProposalPreview = Schema<"ProposalPreview">;
export type ReviewItem = Schema<"ReviewItem">;
export type Conflict = Schema<"Conflict">;
export type Resolution = Schema<"ConflictResolution">;
export type Mapping = Schema<"ParticipationMapping">;
export type AnswerValue = Schema<"AnswerValue">;
export type Choice = Schema<"Choice">;
export type UnresolvedReason = Schema<"UnresolvedReason">;

/** The kinds of resolution a conflict can offer (B7). */
export type ResolutionKind = "keep_journey" | "take_route" | "clear_state" | "reopen" | "remove" | "remap_role" | "remap_kind" | "map_choices";

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

const choiceId = (choice: Choice): string => (typeof choice === "string" ? choice : choice.id);

/** B7: the choices an answer names that `choices` no longer has. */
export function removedChoices(answer: AnswerValue, choices: readonly Choice[]): string[] {
  const remaining = new Set(choices.map(choiceId));
  const named = "single_choice" in answer ? [answer.single_choice] : "multi_choice" in answer ? answer.multi_choice : [];
  return [...new Set(named)].filter((choice) => !remaining.has(choice)).sort();
}

/** A6: a single-valued role cannot hold a direct fill of several entities. */
function tooNarrow(conflict: Extract<Conflict, { about: "role" }>): boolean {
  const route = conflict.route;
  return route != null && route.multi !== true && (conflict.references ?? []).some((reference) => typeof reference === "object" && "fill" in reference && reference.fill.entities.length > 1);
}

/**
 * B7: the kinds of resolution `conflict` offers, in the order a reviewer reads them. Every
 * conflict keeps the journey's side; an edit also takes the route's unless the route's names a
 * node the journey does not hold; a shape change clears the state; an invalidated answer
 * maps, clears, or reopens; a removed role or kind the journey still uses is remapped or
 * removed; a role too narrow for the journey's direct fill clears it. The engine's
 * `Conflict::offers` is the rule; a test checks this against it.
 */
export function offered(conflict: Conflict): ResolutionKind[] {
  switch (conflict.about) {
    case "field":
    case "resource":
      return conflict.dangling === true ? ["keep_journey"] : ["keep_journey", "take_route"];
    case "edge":
    case "participation":
    case "default_owner":
      return ["keep_journey", "take_route"];
    case "shape":
      return conflict.dangling === true ? ["keep_journey"] : ["keep_journey", "clear_state"];
    case "answer":
      return ["keep_journey", "map_choices", "clear_state", "reopen"];
    case "role":
      if (conflict.route == null) {
        return ["keep_journey", "remap_role", "remove"];
      }
      return ["keep_journey", tooNarrow(conflict) ? "clear_state" : "take_route"];
    case "kind":
      return conflict.route == null ? ["keep_journey", "remap_kind", "remove"] : ["keep_journey", "take_route"];
  }
}

/** B7: whether `conflict` offers `resolution` as given: its kind, and a choice map that sends every removed choice to a remaining one. */
export function offers(conflict: Conflict, resolution: Resolution): boolean {
  const kind = kindOf(resolution);
  if (!offered(conflict).includes(kind)) {
    return false;
  }
  if (typeof resolution === "object" && "map_choices" in resolution && conflict.about === "answer") {
    const removed = removedChoices(conflict.answer, conflict.choices);
    const remaining = new Set(conflict.choices.map(choiceId));
    const map = resolution.map_choices.map;
    const keys = Object.keys(map).sort();
    return removed.length > 0 && keys.length === removed.length && keys.every((key, at) => key === removed[at] && remaining.has(map[key] ?? ""));
  }
  return true;
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
    case "violation": {
      const subject = item.violation.at.subject;
      return subject != null && typeof subject === "object" && "node" in subject ? subject.node : undefined;
    }
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

/** B8: the draft with entity `index`'s participations mapped, or the mapping cleared. */
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

/** C14: the diff from `before` to `after`, each list in key order. */
export function graphDiff(before: Graph | null | undefined, after: Graph | null | undefined): GraphDiff {
  const [was, now] = [byKey(before), byKey(after)];
  const added = [...now.keys()].filter((key) => !was.has(key)).sort();
  const removed = [...was.keys()].filter((key) => !now.has(key)).sort();
  const changed: Changed = {};
  for (const [key, node] of now) {
    const old = was.get(key);
    const fields = old === undefined ? [] : fieldsDiffering(old, node);
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

/** Whether a diff changes nothing. */
export function unchanged(diff: GraphDiff): boolean {
  return diff.added.length + diff.removed.length + Object.keys(diff.changed).length + diff.edgesAdded.length + diff.edgesRemoved.length === 0;
}

/**
 * C14: the graph a diff is drawn over: the graph after, with every node it removed put back
 * where it was, its removed edges with it, so a removal is seen in place rather than missing.
 * It carries no state: the canvas draws structure, and the frontier after is listed beside it.
 */
export function unionGraph(before: Graph | null | undefined, after: Graph | null | undefined): Graph {
  const now = byKey(after);
  const was = byKey(before);
  const kept = [...now.values()];
  const removed = [...was.values()].filter((node) => !now.has(node.key));
  const drawn = new Set([...kept, ...removed].map((node) => node.key));
  // A removed node's parent is drawn: kept, or removed and put back with it.
  const back = removed.map((node) => ({ ...node, requires: (node.requires ?? []).filter((key) => drawn.has(key)) }));
  // Drawn without state, and without the keys retired by the removals it puts back.
  const structure: Graph = { ...(after ?? before ?? {}), nodes: [...kept, ...back] };
  delete structure.state;
  delete structure.retired_keys;
  return structure;
}

/** I6: why a proposal cannot be applied now, each a reason the screen names. */
export type Blocker = "not_open" | "editing" | "unsaved" | "stale" | "unresolved" | "invalid" | "unreviewed" | "no_preview";

/** What the apply gate reads. */
export interface GateInput {
  proposal: Pick<Proposal, "status" | "revision">;
  /** The proposal revision the reviewer last confirmed reviewing, if any. */
  reviewed: number | undefined;
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
 * choice for every item and no violation, and have been reviewed at the revision it is at,
 * so a refresh or anyone's edit asks for a renewed review. None means it may be applied.
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
  if (input.reviewed !== input.proposal.revision) {
    blockers.push("unreviewed");
  }
  return blockers;
}

/** What moves a reviewer's confirmation. */
export type ReviewEvent =
  | { event: "marked"; revision: number }
  | { event: "refreshed"; proposal: Pick<Proposal, "revision"> }
  | { event: "saved"; proposal: Pick<Proposal, "revision"> };

/**
 * I6: the revision a reviewer has confirmed after `event`. Marking confirms the revision
 * reviewed. A refresh drafts the proposal again on a new base and a save changes it, each at a
 * new revision, so neither carries the confirmation over: the reviewer reviews again.
 */
export function reviewedAfter(reviewed: number | undefined, event: ReviewEvent): number | undefined {
  switch (event.event) {
    case "marked":
      return event.revision;
    case "refreshed":
    case "saved":
      return reviewed;
  }
}

/** I6: the patches that moved the destination from `expected` to `current`: the last ones of its history. */
export function intervening<P>(patches: readonly P[], expected: number, current: number): P[] {
  const count = Math.max(0, current - expected);
  return count === 0 ? [] : patches.slice(-count);
}

/** The kinds of node a breakdown adds (B10: deliverables and actions). */
export const PIECE_KINDS = ["action", "deliverable"] as const;

/** B10: whether a node can be broken down: a non-group node that is not a leaf-only kind. */
export function breakable(node: Pick<GraphNode, "kind">): boolean {
  return node.kind === "deliverable" || node.kind === "action";
}

/** C14: how the canvas marks one node a proposal touches. */
export interface DiffMark {
  tone: "good" | "warn" | "bad" | "plain" | "accent";
  label: string;
}

/** The words of each mark, so the canvas, its legend, and the list say the same. */
export const DIFF_LABELS = {
  added: "added",
  removed: "removed",
  changed: "changed",
  conflict: "conflict",
  orphan: "orphaned, kept",
  orphanRemoved: "orphaned, removed",
  excluded: "left out",
} as const;

/**
 * C14: the canvas's mark for each node the proposal touches: added, removed, or changed by the
 * diff; a conflict, an orphan, or a node left out of a saved route by its items, which say
 * more than the diff and so win over it.
 */
export function diffMarks(diff: GraphDiff, reviewItems: readonly ReviewItem[]): Record<string, DiffMark> {
  const marks: Record<string, DiffMark> = {};
  for (const key of diff.added) {
    marks[key] = { tone: "good", label: DIFF_LABELS.added };
  }
  for (const key of Object.keys(diff.changed)) {
    marks[key] = { tone: "accent", label: DIFF_LABELS.changed };
  }
  for (const key of diff.removed) {
    marks[key] = { tone: "bad", label: DIFF_LABELS.removed };
  }
  for (const item of reviewItems) {
    if (item.item === "orphan") {
      marks[item.node] = item.keep ? { tone: "warn", label: DIFF_LABELS.orphan } : { tone: "bad", label: DIFF_LABELS.orphanRemoved };
    } else if (item.item === "exclusion" && item.excluded) {
      marks[item.node] = { tone: "plain", label: DIFF_LABELS.excluded };
    } else if (item.item === "conflict") {
      const node = conflictNode(item.conflict);
      if (node !== undefined) {
        marks[node] = { tone: "bad", label: DIFF_LABELS.conflict };
      }
    }
  }
  return marks;
}

/**
 * C14: the items still needing a choice as the review itself can tell, for edits no preview
 * has seen yet: a conflict with no resolution or one it does not offer, an entity with no
 * mapping. The engine's preview also finds mappings that cannot hold together; this never
 * replaces it, it only stands in until the edits are previewed.
 */
export function unresolvedHere(reviewItems: readonly ReviewItem[]): Map<number, UnresolvedReason> {
  const found = new Map<number, UnresolvedReason>();
  reviewItems.forEach((item, index) => {
    if (item.item === "conflict") {
      if (item.resolution == null) {
        found.set(index, "no_choice");
      } else if (!offers(item.conflict, item.resolution)) {
        found.set(index, "not_offered");
      }
    } else if (item.item === "participation" && item.mapping == null) {
      found.set(index, "no_choice");
    }
  });
  return found;
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

/** One drafting attempt: what it asked for, and the ids it was sent under (I6, H5). */
export interface Attempt {
  asked: string;
  id: string;
  patchId: string;
}

/**
 * I6, H5: the ids to send a draft under: those of an attempt whose answer never came, when it
 * asked the same, so a draft that was saved after all is found rather than made twice; fresh
 * ones otherwise.
 */
export function attemptFor(unanswered: Attempt | undefined, asked: string, fresh: () => Omit<Attempt, "asked">): Attempt {
  return unanswered?.asked === asked ? unanswered : { asked, ...fresh() };
}
