// C8: one node's detail, composed in the tab from the journey's document and its local
// derive (ARCHITECTURE, Web UI: the browser derives locally and has every explanation, so
// nothing is fetched for it). It mirrors the server's node detail (crates/service
// projections, `detail`) field for field, plus the decisions that drive the node's values
// (E3), which the panel routes edits through.
import type { Schema } from "@cairn/client";

import type { JourneyView } from "../data/journeys.ts";

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
  state: State;
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

/** A node of the journey by key. */
export function nodeOf(view: Ready, key: string): GraphNode | undefined {
  return (view.journey.graph.nodes ?? []).find((node) => node.key === key);
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
    .map((child) => ({ key: child.key, title: child.title, kind: child.kind, state: recordOf(view, child).state }));
  return {
    node,
    path: [...ancestors, node].map((each) => each.id).join("/"),
    ancestors,
    children,
    record: recordOf(view, node),
    localEdits: state?.local_edits?.[key] ?? [],
    answer: state?.answers?.[key],
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
 * holds back beneath it, which does not block it.
 */
export function isBlocked(derived: NodeDerived, state: State): boolean {
  const inScope = derived.relevance.value !== "not_relevant" && derived.effectively_skipped !== true;
  const held = (derived.blocked_by ?? []).length > 0 || (derived.blocked_through ?? []).length > 0;
  return inScope && !TERMINAL.includes(state) && held;
}

export type FlagTone = "good" | "warn" | "bad" | "plain";

/** D3: the derived flags a node carries (C8), those that are set. */
export function flagsOf(derived: NodeDerived, state: State): { flag: string; tone: FlagTone }[] {
  const flags: { flag: string; tone: FlagTone }[] = [];
  const add = (set: boolean | undefined, flag: string, tone: FlagTone) => {
    if (set === true) {
      flags.push({ flag, tone });
    }
  };
  add(derived.actionable, "actionable", "good");
  add(isBlocked(derived, state), "blocked", "warn");
  add(derived.unassigned, "unassigned", "warn");
  add((derived.stale ?? []).length > 0, "stale", "warn");
  add(derived.overdue, "overdue", "bad");
  add(derived.snoozed != null, "snoozed", "plain");
  add(derived.needs_breakdown, "needs_breakdown", "warn");
  add(derived.effectively_skipped, "effectively skipped", "plain");
  add(derived.auto_reached, "auto-reached", "good");
  add(derived.membership_lost, "membership lost", "warn");
  return flags;
}
