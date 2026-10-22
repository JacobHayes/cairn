// C8: one node's detail, composed in the tab from the journey's document and its local
// derive (ARCHITECTURE, Web UI: the browser derives locally and has every explanation, so
// nothing is fetched for it). It mirrors the server's node detail (crates/service
// projections, `detail`) field for field, plus the decisions that drive the node's values
// (E3), which the panel routes edits through.
import { INITIAL_STATE, MOVES_FROM, TERMINAL_STATES, type Move as EngineMove, type Schema } from "@cairn/client";

import type { JourneyView } from "../data/journeys.ts";
import { isOpen } from "../status/state.ts";
import type { DisplayState } from "../status/words.ts";

import { nodeTitle } from "../ui/gone.ts";

export type Ready = Extract<JourneyView, { status: "ready" }>;
export type GraphNode = Schema<"Node">;
export type NodeKind = Schema<"NodeKind">;

/** C2: every kind, in the order the toggles and pickers list them. */
export const KINDS: NodeKind[] = ["group", "decision", "deliverable", "action", "milestone"];

/** The kinds of work, the pieces a node is broken down into: actions and deliverables (B10). */
export const PIECE_KINDS = ["action", "deliverable"] as const;

/** Whether a node is work (an action or a deliverable): what a person does, where a decision or a milestone is not. */
export function isWork(node: { kind: NodeKind }): boolean {
  return node.kind === "action" || node.kind === "deliverable";
}

/** Whether `node` is a stage: a top-level group. */
export function isStage(node: { kind: NodeKind; parent?: string | null | undefined }): boolean {
  return node.kind === "group" && node.parent == null;
}

/** The kind as people say it: a stage is a stage, not a group. */
export function kindWord(kind: NodeKind, stage: boolean): string {
  return stage ? "stage" : kind;
}
export type State = Schema<"State">;
export type NodeState = Schema<"NodeState">;
export type NodeDerived = Schema<"NodeDerived">;
export type Annotation = Schema<"Annotation">;
export type AnswerValue = Schema<"AnswerValue">;
export type Mutation = Schema<"Mutation">;
export type Guard = Schema<"Guard">;

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

/** A node's title, or words for a node the journey does not hold. */
export function titleOf(view: Ready, key: string): string {
  return nodeTitle(nodeOf(view, key)?.title);
}

/** A node's stored state, or its kind's initial state. */
export function recordOf(view: Ready, node: GraphNode): NodeState {
  return view.journey.graph.state?.nodes?.[node.key] ?? { state: INITIAL_STATE[node.kind], provenance: "local" };
}

/** B6: whether `key` holds a snooze of its own, whether or not it holds (a container's snooze holds its nodes through it). */
export function storedSnooze(view: Ready, key: string): boolean {
  return view.journey.graph.state?.snoozes?.[key] !== undefined;
}

/** E3: the decision whose answer pins `milestone`, if one feeds it. */
export function feedingDecision(view: Ready, milestone: string): GraphNode | undefined {
  return (view.journey.graph.nodes ?? []).find((node) => node.feeds_milestone === milestone);
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

/** D4: the override that lets a move past the `guards` it would fail, with the reason it was given; the one place it is built. */
export function guardBypass(node: string, guards: Schema<"Guard">[], reason: string): Mutation {
  return { op: "apply_override", node, override: { guard_bypass: { guards, reason: reason.trim() } } };
}

/** A transition by name (D1); answering a decision is its own mutation. */
export type Move = Exclude<EngineMove, "answer">;

const MOVE_ORDER: Move[] = ["start", "stop", "complete", "skip", "mark_done", "reopen"];

/** D1: the moves a node of `kind` in `state` can make, in the order the panel offers them. */
export function movesFrom(kind: NodeKind, state: State): Move[] {
  return MOVE_ORDER.filter((move) => MOVES_FROM[kind][move]?.includes(state) === true);
}

/** D1: whether a decision in `state` can be answered (a first answer or a revision). */
export function answerable(kind: NodeKind, state: State): boolean {
  return MOVES_FROM[kind].answer?.includes(state) === true;
}

/** The transition mutation for `move` on `node`; a skip and a mark done carry their reason (D1). */
export function transition(node: string, move: Move, reason = ""): Mutation {
  if (move === "skip") {
    return { op: "transition", node, transition: { skip: { reason } } };
  }
  if (move === "mark_done") {
    return { op: "transition", node, transition: { mark_done: { reason } } };
  }
  return { op: "transition", node, transition: move };
}

/** The move a transition makes: its name, a string or the one key of the object that carries a reason. */
export function moveOf(change: Schema<"Transition">): Move {
  if (typeof change === "string") {
    return change;
  }
  return "skip" in change ? "skip" : "mark_done";
}

/**
 * D1a: how many items (nodes that are not groups) inside `key` that are unfinished and in scope
 * would read skipped if `key` were marked done: the form says it before it is sent. Work marked
 * Keep, and what is inside it, is left out, because it stays open.
 */
export function unfinishedInside(view: Ready, key: string): number {
  const kept = view.journey.graph.state?.overrides ?? {};
  const inside = (node: GraphNode): boolean => {
    // Walks up from the node: it is inside the group when the walk reaches it, unless a keep is met first.
    for (let at: GraphNode | undefined = node; at?.parent != null; at = nodeOf(view, at.parent)) {
      if (kept[at.key]?.keep != null) {
        return false;
      }
      if (at.parent === key) {
        return true;
      }
    }
    return false;
  };
  return (view.journey.graph.nodes ?? []).filter((node) => node.kind !== "group" && isOpen(view.derived.nodes[node.key]) && inside(node)).length;
}

/**
 * D1a: how many items (nodes that are not groups) read skipped through `key`, a skipped
 * container or a group marked done. The card and the sentence both say it, so it is counted
 * here once.
 */
export function skippedThrough(view: Ready, key: string): number {
  return Object.entries(view.derived.nodes).filter(([node, derived]) => derived.skipped_by === key && nodeOf(view, node)?.kind !== "group").length;
}

/** D1: whether a node in `state` is finished (done, skipped, decided). */
export function isTerminal(state: State): boolean {
  return TERMINAL_STATES.includes(state);
}

/** D4: the guards the node's completing transition would fail now, as the engine derived them. */
export function failingGuards(derived: NodeDerived): Guard[] {
  return derived.failing_guards ?? [];
}

/**
 * Gating, D8: a dependency the node waits on: what it is, how it arose, and the ancestor holding
 * it for the node, if one does.
 */
export interface Holding {
  node: string;
  via: Schema<"DependencyVia">;
  holder: string | undefined;
  /** The date it is scheduled for, while a date ahead holds it (D8). */
  date: string | undefined;
}

/**
 * Gating, D8: what the node waits on, as the engine derived it: its own unsatisfied requirements,
 * condition gates and stage opening, then those its ancestors hold for it, nearest first, each
 * once. What it contains does not hold it.
 */
export function waitsOn(view: Ready, key: string): Holding[] {
  return (view.derived.nodes[key]?.waits_on.entries ?? []).map((blocker) => {
    const via = blocker.via;
    const holder = typeof via !== "object" ? undefined : "inherited" in via ? via.inherited.ancestor : "condition" in via ? via.condition.condition_on : "stage_opening" in via ? via.stage_opening.group : undefined;
    return { node: blocker.node, via, holder: holder === key ? undefined : holder, date: blocker.date ?? undefined };
  });
}

/**
 * D8: the date a scheduled node waits for: a milestone's effective date (its pin, or the date it
 * marks itself done), else its own not-before date. None for a node no date holds.
 */
export function scheduledDate(view: Ready, key: string): string | undefined {
  const derived = view.derived.nodes[key];
  if (derived?.display_state !== "scheduled") {
    return undefined;
  }
  return nodeOf(view, key)?.kind === "milestone" ? derived.dates.effective_date?.date : (derived.dates.not_before ?? undefined);
}

/** Gating, D8: how many more the node waits on than `waitsOn` lists (the engine cuts the list at its explanation limit and says the total). */
export function waitsMore(view: Ready, key: string): number {
  const waits = view.derived.nodes[key]?.waits_on;
  return waits === undefined ? 0 : Math.max(0, waits.total - waits.entries.length);
}

/** Milestones first, then by node key: the order a list of things a node waits on is shown in. */
export function milestonesFirst<Item>(view: Ready, items: readonly Item[], keyOf: (item: Item) => string): Item[] {
  const milestone = (item: Item) => Number(nodeOf(view, keyOf(item))?.kind === "milestone");
  return [...items].sort((left, right) => milestone(right) - milestone(left) || keyOf(left).localeCompare(keyOf(right)));
}

/**
 * C5, C6, C10: a node's place on the acting frontier, one-based, in the engine's order (rank,
 * slack, gravity, key); a ranked node off the frontier has none. One meaning of "#N" everywhere.
 */
export function rankPlaces(view: Ready): Map<string, number> {
  let found = PLACES.get(view.derived);
  if (found === undefined) {
    found = new Map(view.derived.acting_frontier.map((key, at) => [key, at + 1]));
    PLACES.set(view.derived, found);
  }
  return found;
}

const PLACES = new WeakMap<Ready["derived"], Map<string, number>>();

export type FlagTone = "good" | "warn" | "bad" | "plain";

/**
 * D3: the derived flags a node carries (C8) that are real attention, those that are set. The one list every surface (inspector, rows, cards) renders. What
 * its display state already says (blocked, snoozed, skipped, actionable, done by its date) is not
 * a flag (D8); started early is the engine's.
 */
export function flagsOf(derived: NodeDerived): { flag: string; tone: FlagTone }[] {
  const flags: { flag: string; tone: FlagTone }[] = [];
  const add = (set: boolean | undefined, flag: string, tone: FlagTone) => {
    if (set === true) {
      flags.push({ flag, tone });
    }
  };
  add(derived.started_early, "started early", "plain");
  add(derived.unassigned, "unassigned", "warn");
  add(derived.overdue, "overdue", "bad");
  add(derived.dates.shortfall != null, "short of days", "bad");
  add((derived.stale ?? []).length > 0, "stale", "warn");
  add(derived.needs_breakdown, "needs breakdown", "warn");
  add(derived.membership_lost, "membership lost", "warn");
  return flags;
}
