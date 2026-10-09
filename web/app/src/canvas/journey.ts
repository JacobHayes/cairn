// What a journey adds to its canvas's cards (C1, C2, C5, C6): each card's display state (D8), owner, due
// date and its urgency, latest start and slack, a decision's answer, its relevance look, the
// border weight its gravity gives it, the "I am here" marks and the top few rank badges, its
// derived flags, and a container's roll-ups. All read from the document and its local derive.
import { nodeOf, recordOf, startedEarly, type GraphNode, type NodeDerived, type Ready } from "../detail/model.ts";
import { answerText, entityName } from "../detail/sections.tsx";
import type { DisplayState } from "../status/words.ts";
import {
  RANK_BADGE_COUNT,
  borderFor,
  dueTone,
  type CardBadge,
  type CardState,
  type JourneyExtras,
  type LevelNode,
  type Looks,
} from "./model.ts";

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

/** In scope and not finished: what gravity is normalized over (Priority). */
function open(view: Ready, node: GraphNode): boolean {
  const derived = view.derived.nodes[node.key];
  return derived !== undefined && derived.relevance.value !== "not_relevant" && !finishedState(derived.display_state);
}

/** C6: the largest gravity among the journey's open nodes. */
export function gravityMaxOf(view: Ready): number {
  return (view.journey.graph.nodes ?? [])
    .filter((node) => open(view, node))
    .reduce((max, node) => Math.max(max, view.derived.nodes[node.key]?.gravity ?? 0), 0);
}

/** C1: a node's owners by name, or "unassigned". */
function ownersOf(view: Ready, key: string): string {
  const owners = view.derived.nodes[key]?.participations?.["k_owner"]?.entities ?? [];
  return owners.length === 0 ? "unassigned" : owners.map((entity) => entityName(view, entity)).join(", ");
}

/**
 * D3 flags and C2 roll-up badges a card carries, quiet ones left to the card's other lines. What
 * its state chip already says (blocked, snoozed, skipped) is not repeated; `startedEarly` is
 * started work whose gates are not met.
 */
export function badgesOf(derived: NodeDerived, startedEarly: boolean, at: LevelNode): CardBadge[] {
  const badges: CardBadge[] = [];
  const add = (set: boolean | undefined, flag: string, tone: CardBadge["tone"]) => {
    if (set === true) {
      badges.push({ flag, tone });
    }
  };
  add(startedEarly, "started early", "plain");
  add(derived.overdue, "overdue", "bad");
  add(derived.dates.shortfall != null, "shortfall", "bad");
  add((derived.stale ?? []).length > 0, "stale", "warn");
  add(derived.needs_breakdown, "needs breakdown", "warn");
  add(at.kept_work_pending, "kept work pending", "warn");
  const roll = at.roll_up;
  add(roll?.ready_to_finish, "children complete, ready to finish", "good");
  add(roll?.children_active, "children active", "plain");
  add(roll?.all_blocked, "all blocked", "warn");
  add(roll?.decision_needed, "decision needed", "warn");
  add(roll?.needs_breakdown && derived.needs_breakdown !== true, "child needs breakdown", "warn");
  return badges;
}

interface Context {
  view: Ready;
  gravityMax: number;
  ranked: string[];
  mine: Set<string>;
  frontier: Set<string>;
}

function cardState(context: Context, node: GraphNode, at: LevelNode): CardState {
  const { view } = context;
  const derived = view.derived.nodes[node.key];
  if (derived === undefined) {
    throw new Error(`the derive has no node ${node.key}`);
  }
  const state = stateOf(view, node);
  const stored = recordOf(view, node).state;
  const early = startedEarly(derived, stored);
  const { due, latest_start: latestStart, slack_days: slackDays } = derived.dates;
  const answer = view.journey.graph.state?.answers?.[node.key];
  const rank = context.ranked.indexOf(node.key);
  const roll = at.roll_up;
  const finished = finishedState(state);
  return {
    state,
    finished,
    owner: ownersOf(view, node.key),
    relevance: derived.relevance.value,
    due: due == null ? undefined : { date: due.date, tone: dueTone(due.date, view.derived.today, derived.overdue === true, finished) },
    latestStart: latestStart?.date,
    slackDays: slackDays ?? undefined,
    answer: answer === undefined ? undefined : answerText(view, answer, node),
    borderPx: borderFor(derived.gravity, context.gravityMax, open(view, node)),
    gravity: derived.gravity,
    leverage: derived.leverage,
    here: {
      frontier: context.frontier.has(node.key),
      active: stored === "active" && !finishedState(state),
      startedEarly: early,
      mine: context.mine.has(node.key),
    },
    rank: rank < 0 ? undefined : rank + 1,
    badges: badgesOf(derived, early, at),
    children:
      roll == null
        ? undefined
        : {
            gravity: roll.peak_gravity?.gravity ?? undefined,
            slackDays: roll.min_child_slack_days ?? undefined,
            owners: (roll.owners ?? []).map((entity) => entityName(view, entity)),
          },
  };
}

/** C1, C5, C6: a journey's looks for its cards; `extras` are its rank order and the viewer's items. */
export function journeyLooks(view: Ready, extras: JourneyExtras): Looks {
  const context: Context = {
    view,
    gravityMax: gravityMaxOf(view),
    ranked: extras.ranked.slice(0, RANK_BADGE_COUNT),
    mine: new Set(extras.mine),
    frontier: new Set(view.derived.frontier),
  };
  return {
    card: (node, at) => cardState(context, node, at),
    finished: (key) => {
      const node = nodeOf(view, key);
      return node !== undefined && finishedState(stateOf(view, node));
    },
  };
}
