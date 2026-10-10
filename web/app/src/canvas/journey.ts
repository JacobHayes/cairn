// What a journey adds to its canvas's cards (C1, C2, C5, C6): each card's display state (D8),
// its one date and the owner when it is the viewer's or missing, a decided decision's answer, a
// container's progress and roll-up badge, what a conditional node depends on, the "I am here"
// marks, the rank order, and the numbers the Signals lens shows. All read from the document and
// its local derive; the words are words.ts's.
import { nodeOf, recordOf, startedEarly, type GraphNode, type NodeDerived, type Ready } from "../detail/model.ts";
import { answerText, entityName } from "../detail/sections.tsx";
import { rollups } from "../plan/tree.ts";
import type { DisplayState } from "../status/words.ts";
import { currentStages, isStage } from "./ladder.ts";
import type { CardBadge, CardBody, CardState, JourneyExtras, LevelNode, Looks, Signals } from "./model.ts";
import { dependsWords, firstParagraph, footWords, snoozeTarget } from "./words.ts";

/** A node's display state (D8): the engine's one state, which the canvas shows as it is. */
export function stateOf(view: Ready, node: GraphNode): DisplayState {
  const derived = view.derived.nodes[node.key];
  if (derived === undefined) {
    throw new Error(`the derive has no node ${node.key}`);
  }
  return derived.display_state;
}

/** Done or skipped: nothing is left to do on it. */
function finishedState(state: DisplayState): boolean {
  return state === "done" || state === "skipped";
}

/** C1: a node's owners by name, or none. */
function ownersOf(view: Ready, key: string): string[] {
  const owners = view.derived.nodes[key]?.participations?.["k_owner"]?.entities ?? [];
  return owners.map((entity) => entityName(view, entity));
}

/** C2: the one roll-up badge a container carries, in the fixed order: a decision to make, late, all blocked, ready to finish, needs breakdown. */
export function rollUpBadge(derived: NodeDerived, at: LevelNode): CardBadge | undefined {
  const roll = at.roll_up;
  if (roll?.decision_needed === true) {
    return { flag: "decision to make", tone: "warn" };
  }
  if (derived.overdue === true) {
    return { flag: "late", tone: "bad" };
  }
  if (roll?.all_blocked === true) {
    return { flag: "all blocked", tone: "warn" };
  }
  if (roll?.ready_to_finish === true) {
    return { flag: "ready to finish", tone: "good" };
  }
  if (roll?.needs_breakdown === true || derived.needs_breakdown === true) {
    return { flag: "needs breakdown", tone: "warn" };
  }
  return undefined;
}

interface Context {
  view: Ready;
  ranked: string[];
  mine: Set<string>;
  owned: Set<string>;
  frontier: Set<string>;
  current: Set<string>;
  children: Map<string, string[]>;
}

/** What a conditional node depends on, as one sentence: `If Who runs testing? = Partner, once Budget is answered`. */
function dependsBody(context: Context, node: GraphNode, derived: NodeDerived): CardBody | undefined {
  const { view } = context;
  const nodes = view.journey.graph.nodes ?? [];
  const source = derived.relevance.condition_on == null ? node : (nodeOf(view, derived.relevance.condition_on) ?? node);
  const pending = (derived.relevance.pending_on ?? []).map((key) => nodeOf(view, key)?.title ?? key);
  const reads = (derived.relevance.decisions ?? []).map((key) => nodeOf(view, key)?.title ?? key);
  const words = dependsWords(nodes, source) ?? (reads.length === 0 ? undefined : reads.join(" and "));
  if (words === undefined) {
    return undefined;
  }
  return { kind: "depends", text: pending.length === 0 ? `If ${words}` : `If ${words}, once ${pending.join(" and ")} ${pending.length === 1 ? "is" : "are"} answered` };
}

function bodyOf(context: Context, node: GraphNode, derived: NodeDerived, at: LevelNode, state: DisplayState): CardBody | undefined {
  const { view } = context;
  if (state === "conditional") {
    return dependsBody(context, node, derived);
  }
  if (state === "not_relevant") {
    return undefined;
  }
  if (node.kind === "decision") {
    const answer = view.journey.graph.state?.answers?.[node.key];
    if (state !== "done" || answer === undefined) {
      return undefined;
    }
    const rationale = view.journey.graph.state?.rationales?.[node.key];
    return { kind: "answer", text: answerText(view, answer, node), rationale: rationale === undefined || rationale === "" ? undefined : firstParagraph(rationale) };
  }
  if (context.children.has(node.key)) {
    const { done, total } = rollups(view).get(node.key) ?? { done: 0, total: 0 };
    return total === 0 ? undefined : { kind: "progress", done, total, badge: rollUpBadge(derived, at) };
  }
  return undefined;
}

/** The foot's date, or where a snooze ends, or the container a snooze holds through (7.4). */
function footOf(context: Context, node: GraphNode, derived: NodeDerived, state: DisplayState): CardState["foot"] {
  const { view } = context;
  if (derived.snoozed_via != null) {
    return { words: `z via ${nodeOf(view, derived.snoozed_via)?.title ?? derived.snoozed_via}`, tone: "plain" };
  }
  const target = derived.snoozed;
  if (state === "snoozed" && target != null) {
    const until = snoozeTarget(view, derived) ?? "";
    return { words: "date" in target ? `Snoozed until ${until}` : `Snoozed until ${until} is done`, tone: "plain" };
  }
  const { due, latest_start: latestStart, effective_date: effective } = derived.dates;
  return footWords({
    kind: node.kind,
    state,
    today: view.derived.today,
    due: due?.date,
    latestStart: latestStart?.date,
    overdue: derived.overdue === true,
    reached: effective?.date,
  });
}

function signalsOf(context: Context, node: GraphNode, derived: NodeDerived, at: LevelNode, rank: number | undefined, finished: boolean): Signals {
  const container = context.children.has(node.key);
  const slack = container ? at.roll_up?.min_child_slack_days : derived.dates.slack_days;
  return {
    rank,
    gravity: finished || derived.relevance.value === "not_relevant" ? undefined : at.roll_up?.subtree_gravity ?? derived.gravity,
    unlocks: context.frontier.has(node.key) ? { count: derived.unlocks_from.total, weighted: derived.unlocks } : undefined,
    slackDays: finished || slack == null ? undefined : slack,
  };
}

function cardState(context: Context, node: GraphNode, at: LevelNode): CardState {
  const { view } = context;
  const derived = view.derived.nodes[node.key];
  if (derived === undefined) {
    throw new Error(`the derive has no node ${node.key}`);
  }
  const state = stateOf(view, node);
  const stored = recordOf(view, node).state;
  const finished = finishedState(state);
  const index = context.ranked.indexOf(node.key);
  const rank = index < 0 ? undefined : index + 1;
  const names = ownersOf(view, node.key);
  const mine = context.mine.has(node.key);
  const missing = derived.unassigned === true && !finished && state !== "not_relevant";
  return {
    state,
    finished,
    relevance: derived.relevance.value,
    foot: state === "not_relevant" ? undefined : footOf(context, node, derived, state),
    owner: context.owned.has(node.key) ? { words: "You", missing: false } : missing ? { words: "Unassigned", missing: true } : undefined,
    owners: names.length === 0 ? "unassigned" : names.join(", "),
    body: bodyOf(context, node, derived, at, state),
    here: {
      frontier: context.frontier.has(node.key),
      active: stored === "active" && !finished,
      startedEarly: startedEarly(derived, stored),
      mine,
    },
    rank,
    signals: signalsOf(context, node, derived, at, rank, finished),
    current: context.current.has(node.key) && isStage(node),
    origin: recordOf(view, node).provenance,
  };
}

/** C1, C5, C6: a journey's looks for its cards; `extras` are its rank order and the viewer's items. */
export function journeyLooks(view: Ready, extras: JourneyExtras): Looks {
  const nodes = view.journey.graph.nodes ?? [];
  const children = new Map<string, string[]>();
  for (const node of nodes) {
    if (node.parent != null) {
      children.set(node.parent, [...(children.get(node.parent) ?? []), node.key]);
    }
  }
  const active = nodes.filter((node) => recordOf(view, node).state === "active" && !finishedState(stateOf(view, node))).map((node) => node.key);
  const context: Context = {
    view,
    ranked: extras.ranked,
    mine: new Set(extras.mine),
    owned: new Set(extras.owned),
    frontier: new Set(view.derived.frontier),
    current: currentStages(nodes, view.derived.acting_frontier, active),
    children,
  };
  return {
    card: (node, at) => cardState(context, node, at),
    finished: (key) => {
      const node = nodeOf(view, key);
      return node !== undefined && finishedState(stateOf(view, node));
    },
  };
}
