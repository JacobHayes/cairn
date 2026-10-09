// C8: one node's detail, composed in the tab from the journey's document and its local
// derive (ARCHITECTURE, Web UI: the browser derives locally and has every explanation, so
// nothing is fetched for it). It mirrors the server's node detail (crates/service
// projections, `detail`) field for field, plus the decisions that drive the node's values
// (E3), which the panel routes edits through.
import type { Schema } from "@cairn/client";

import type { JourneyView } from "../data/journeys.ts";
import type { DisplayState } from "../status/words.ts";

export type Ready = Extract<JourneyView, { status: "ready" }>;
export type GraphNode = Schema<"Node">;
export type NodeKind = Schema<"NodeKind">;
export type State = Schema<"State">;
export type NodeState = Schema<"NodeState">;
export type NodeDerived = Schema<"NodeDerived">;
export type Annotation = Schema<"Annotation">;
export type AnswerValue = Schema<"AnswerValue">;
export type Mutation = Schema<"Mutation">;

/** One child in a node's checklist. */
export interface Child {
  key: string;
  title: string;
  kind: NodeKind;
  /** Its stored state: which transition a checkbox would make. */
  state: State;
  /** D8: the state it shows. */
  displayState: DisplayState;
}

/** C8: everything the node detail panel shows about one node. */
export interface NodeDetail {
  node: GraphNode;
  /** Where it sits: its ancestors' ids and its own, joined by `/`. */
  path: string;
  /** Its ancestors, root first. */
  ancestors: GraphNode[];
  children: Child[];
  /** Its stored state, provenance, and recorded dates. */
  record: NodeState;
  localEdits: Schema<"LocalEdit">[];
  answer: AnswerValue | undefined;
  /** B2: the markdown reason its recorded answer was given with. */
  rationale: string | undefined;
  /** Its direct pin (F2); a pin its feeding decision gives is `fedBy`'s answer (E3). */
  pin: string | undefined;
  overrides: Schema<"Overrides"> | undefined;
  /** Its notes and links (G1), in key order. */
  annotations: Annotation[];
  derived: NodeDerived;
  /** E3: the `feeds_milestone` decision that pins this milestone, if any. */
  fedBy: GraphNode | undefined;
}

/** D1: each kind's initial state, which a node with no stored state is in. */
export const INITIAL_STATE: Record<NodeKind, State> = {
  deliverable: "todo",
  action: "todo",
  decision: "open",
  milestone: "pending",
  group: "derived",
};

/**
 * Each journey's nodes by key, built once per document's node list (a document is never
 * changed in place: a new revision is a new list), so a screen naming every node it lists
 * looks each up in constant time rather than scanning the journey per name.
 */
const NODE_INDEXES = new WeakMap<GraphNode[], Map<string, GraphNode>>();

const NO_NODES: GraphNode[] = [];

/** A node of the journey by key. */
export function nodeOf(view: Ready, key: string): GraphNode | undefined {
  const nodes = view.journey.graph.nodes ?? NO_NODES;
  let index = NODE_INDEXES.get(nodes);
  if (index === undefined) {
    index = new Map(nodes.map((node) => [node.key, node]));
    NODE_INDEXES.set(nodes, index);
  }
  return index.get(key);
}

/** A node's title, or its key when the journey does not hold it. */
export function titleOf(view: Ready, key: string): string {
  return nodeOf(view, key)?.title ?? key;
}

/** A node's stored state, or its kind's initial state. */
export function recordOf(view: Ready, node: GraphNode): NodeState {
  return view.journey.graph.state?.nodes?.[node.key] ?? { state: INITIAL_STATE[node.kind], provenance: "local" };
}

/** E3: the decision whose answer pins `milestone`, if one feeds it. */
export function feedingDecision(view: Ready, milestone: string): GraphNode | undefined {
  return (view.journey.graph.nodes ?? []).find((node) => node.feeds_milestone === milestone);
}

/** E3: the decision that fills `role`, if one does. */
export function fillingDecision(view: Ready, role: string): GraphNode | undefined {
  return (view.journey.graph.nodes ?? []).find((node) => node.fills_role === role);
}

function ancestorsOf(view: Ready, node: GraphNode): GraphNode[] {
  const ancestors: GraphNode[] = [];
  let parent = node.parent ?? undefined;
  while (parent !== undefined) {
    const found = nodeOf(view, parent);
    if (found === undefined || ancestors.includes(found)) {
      break;
    }
    ancestors.unshift(found);
    parent = found.parent ?? undefined;
  }
  return ancestors;
}

/** C8: node `key`'s detail, or undefined when the journey does not hold it. */
export function nodeDetail(view: Ready, key: string): NodeDetail | undefined {
  const node = nodeOf(view, key);
  const derived = view.derived.nodes[key];
  if (node === undefined || derived === undefined) {
    return undefined;
  }
  const state = view.journey.graph.state;
  const ancestors = ancestorsOf(view, node);
  const children = (view.journey.graph.nodes ?? [])
    .filter((child) => child.parent === key)
    .map((child) => ({
      key: child.key,
      title: child.title,
      kind: child.kind,
      state: recordOf(view, child).state,
      displayState: view.derived.nodes[child.key]?.display_state ?? "ready",
    }));
  return {
    node,
    path: [...ancestors, node].map((each) => each.id).join("/"),
    ancestors,
    children,
    record: recordOf(view, node),
    localEdits: state?.local_edits?.[key] ?? [],
    answer: state?.answers?.[key],
    rationale: state?.rationales?.[key],
    pin: state?.pins?.[key],
    overrides: state?.overrides?.[key],
    annotations: (state?.annotations ?? []).filter((annotation) => annotation.body.node === key),
    derived,
    fedBy: node.kind === "milestone" ? feedingDecision(view, key) : undefined,
  };
}

/** A transition by name (D1); answering a decision is its own mutation. */
export type Move = "start" | "stop" | "complete" | "skip" | "reopen" | "reach";

/** D1: the states each kind's moves leave from (crates/engine `transition::rows`). */
const MOVES_FROM: Record<NodeKind, Partial<Record<Move, State[]>>> = {
  deliverable: {
    start: ["todo"],
    stop: ["active"],
    complete: ["todo", "active"],
    skip: ["todo", "active"],
    reopen: ["done", "skipped"],
  },
  action: {
    start: ["todo"],
    stop: ["active"],
    complete: ["todo", "active"],
    skip: ["todo", "active"],
    reopen: ["done", "skipped"],
  },
  decision: { skip: ["open"], reopen: ["decided", "skipped"] },
  milestone: { reach: ["pending"], skip: ["pending"], reopen: ["reached", "skipped"] },
  group: { skip: ["derived"], reopen: ["skipped"] },
};

const MOVE_ORDER: Move[] = ["start", "stop", "complete", "reach", "skip", "reopen"];

/** D1: the moves a node of `kind` in `state` can make, in the order the panel offers them. */
export function movesFrom(kind: NodeKind, state: State): Move[] {
  return MOVE_ORDER.filter((move) => MOVES_FROM[kind][move]?.includes(state) === true);
}

/** D1: whether a decision in `state` can be answered (a first answer or a revision). */
export function answerable(kind: NodeKind, state: State): boolean {
  return kind === "decision" && (state === "open" || state === "decided");
}

/** The transition mutation for `move` on `node`; a skip carries its reason (D1). */
export function transition(node: string, move: Move, reason = ""): Mutation {
  if (move === "skip") {
    return { op: "transition", node, transition: { skip: { reason } } };
  }
  return { op: "transition", node, transition: move };
}

/** D1: the states a node is finished in; a group's `derived` is not one. */
const TERMINAL: State[] = ["done", "skipped", "decided", "reached"];

/** D1: whether a node in `state` is finished (done, skipped, decided, reached). */
export function isTerminal(state: State): boolean {
  return TERMINAL.includes(state);
}

/**
 * Gating, Blocked: a node in scope and not finished with an unsatisfied dependency of its own
 * or through an ancestor. A finished or not-relevant node's `blocked_by` lists what it still
 * holds back beneath it, which does not block it. It names the gates held, not the state a
 * person reads: that is the node's display state (D8), where a container is never blocked by
 * its own children.
 */
export function isBlocked(derived: NodeDerived): boolean {
  const inScope = derived.relevance.value !== "not_relevant";
  const finished = derived.display_state === "done" || derived.display_state === "skipped";
  const held = (derived.blocked_by ?? []).length > 0 || (derived.blocked_through ?? []).length > 0;
  return inScope && !finished && held;
}

/**
 * Work started before its own gates are met: stored `active` while a requirement of its own
 * or an ancestor's is unsatisfied. A container's unfinished children are not a gate (D8).
 */
export function startedEarly(derived: NodeDerived, state: State): boolean {
  const gates = (derived.blocked_by ?? []).some((blocker) => blocker.via !== "containment") || (derived.blocked_through ?? []).length > 0;
  return state === "active" && gates && isBlocked(derived);
}

/**
 * Gating, D4: the open decisions an undecided node's relevance waits on: those still to be
 * answered (open, relevant, and not under a skip) that each condition leaving it undecided
 * reads, its own and its ancestors', up to a force include; empty when the node is not
 * undecided. Finishing it is accepted, with the warning that it may not apply.
 */
export function unansweredOf(view: Ready, key: string): string[] {
  const open = (decision: string) => {
    const node = nodeOf(view, decision);
    const found = view.derived.nodes[decision];
    return (
      node !== undefined &&
      found !== undefined &&
      found.relevance.value === "relevant" &&
      found.effectively_skipped !== true &&
      recordOf(view, node).state === "open"
    );
  };
  const waiting = new Set<string>();
  // Up the tree while undecided: a relevant node has nothing undecided above it.
  let current: string | undefined = key;
  let relevance = view.derived.nodes[key]?.relevance;
  while (current !== undefined && relevance?.value === "undecided") {
    // The node's own condition is undecided exactly when it produced the value.
    if (relevance.condition_on === undefined) {
      (relevance.decisions ?? []).filter(open).forEach((decision) => waiting.add(decision));
    }
    current = nodeOf(view, current)?.parent ?? undefined;
    relevance = current === undefined ? undefined : view.derived.nodes[current]?.relevance;
  }
  return [...waiting].sort();
}

export type FlagTone = "good" | "warn" | "bad" | "plain";

/**
 * D3: the derived flags a node carries (C8) that are real attention, those that are set. What
 * its display state already says (blocked, snoozed, skipped, actionable, auto-reached) is not
 * a flag (D8); `state` is the stored one, for work started before its gates are met.
 */
export function flagsOf(derived: NodeDerived, state: State): { flag: string; tone: FlagTone }[] {
  const flags: { flag: string; tone: FlagTone }[] = [];
  const add = (set: boolean | undefined, flag: string, tone: FlagTone) => {
    if (set === true) {
      flags.push({ flag, tone });
    }
  };
  add(startedEarly(derived, state), "started early", "plain");
  add(derived.unassigned, "unassigned", "warn");
  add((derived.stale ?? []).length > 0, "stale", "warn");
  add(derived.overdue, "overdue", "bad");
  add(derived.needs_breakdown, "needs_breakdown", "warn");
  add(derived.membership_lost, "membership lost", "warn");
  return flags;
}
