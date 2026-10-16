// C2, C9, C10, C11, C13: the view state (view.ts) as each view's engine requests and emphasis.
// Hiding is one level request on every view: Depth's kinds and Stages' top-level rule, the
// relevance classes, the Filter and the search, so what is hidden rolls up, its edges re-target,
// and the hidden-prerequisites marker holds wherever it is drawn. The list's table and the walk
// through read their rows beside it and keep the ones the level draws. With Fade on the level is
// the Actions one and Depth dims what it would have hidden. Frontier lights a set of nodes and
// Fade's depth dims another; a node out of focus for either is dimmed, once. Pure, so the unit
// tests read the rules without a page.
import type { ListQuery, ProjectionRequest } from "@cairn/wasm";

import type { SortBy } from "../acting/address.ts";
import { currentStages, drawnStep, kindsAt, leftOutAt, STAGES_TOP } from "../canvas/ladder.ts";
import type { LevelDisplay, Step } from "../canvas/model.ts";
import { recordOf, type GraphNode, type Ready } from "../detail/model.ts";
import type { Badge } from "../screens/badge.ts";
import type { Screen } from "./address.ts";
import { FILTER_FLAGS, type ViewState } from "./view.ts";

export type LevelRequest = Extract<ProjectionRequest, { projection: "level" }>;

/** C9: the list flags the Filter asks for: Only mine first, then the flags in the Filter's order. */
function flagsOf(state: ViewState): NonNullable<LevelRequest["flags"]> {
  return [...(state.filter.mine ? (["mine"] as const) : []), ...FILTER_FLAGS.filter((flag) => state.filter.flags.includes(flag))];
}

/**
 * C1, C2, C9: the level every view draws from: the step's kinds (at Stages, only under a top-level
 * stage or milestone), the containers collapsed, the relevance classes shown, and the Filter and
 * search, which keep a node that matches or holds a match. Parts at their default are left out.
 */
export function levelRequestOf(state: ViewState, step: Step, collapsed: readonly string[]): LevelRequest {
  const { filter } = state;
  const display: LevelDisplay[] = ["relevant", ...(filter.conditional ? (["conditional"] as const) : []), ...(filter.notRelevant ? (["not_relevant"] as const) : [])];
  const flags = flagsOf(state);
  const text = state.text.trim();
  return {
    projection: "level",
    shown: kindsAt(step),
    ...(step === "stages" ? { top: STAGES_TOP } : {}),
    ...(collapsed.length === 0 ? {} : { collapsed: [...collapsed] }),
    ...(display.length === 3 ? {} : { display }),
    ...(flags.length === 0 ? {} : { flags }),
    ...(filter.owner === undefined ? {} : { owner: filter.owner }),
    ...(text === "" ? {} : { text }),
  };
}

/**
 * C9, C13: the list table's read: every row, sorted (by rank when no column is). It carries no
 * kinds and no filter: the table keeps the rows the level draws, so a depth, a search or a filter
 * re-reads only the level.
 */
export function tableQueryOf(sort: SortBy | undefined, cursor?: number): ListQuery {
  return { sort: sort ?? "rank", ...(cursor === undefined ? {} : { cursor }) };
}

/**
 * C10, C11: the walk through's cards: the acting frontier in rank order (as the next list ranks
 * it), narrowed by the Filter and the search; at Decisions depth only the decisions (the decision
 * walkthrough: the level there also draws milestones, which a walk of decisions leaves out).
 */
export function walkQueryOf(state: ViewState, depth: Step): ListQuery {
  const text = state.text.trim();
  return {
    flags: ["next_up", ...flagsOf(state)],
    sort: "rank",
    ...(depth === "decisions" ? { kinds: ["decision" as const] } : {}),
    ...(state.filter.owner === undefined ? {} : { owner: state.filter.owner }),
    ...(text === "" ? {} : { text }),
  };
}

/**
 * C10: the nodes Frontier lights: the acting frontier. None when Frontier is off, and on the walk
 * through and the summary (the walk's every card is on the frontier).
 */
export function litOf(screen: Screen, state: ViewState, ready: Ready): ReadonlySet<string> | undefined {
  return screen === "walk" || screen === "summary" || !state.frontier ? undefined : new Set(ready.derived.acting_frontier);
}

/** C2, 5.1: the stages that are current in `ready`: those holding an acting-frontier or active node. */
export function currentStagesOf(ready: Ready): Set<string> {
  const nodes = ready.journey.graph.nodes ?? [];
  const active = nodes.filter((node) => recordOf(ready, node).state === "active").map((node) => node.key);
  return currentStages(nodes, ready.derived.acting_frontier, active);
}

/** `base` with every container above a node in it: a row that holds a lit node is lit too. */
export function lightsUp(nodes: readonly GraphNode[], base: ReadonlySet<string>): ReadonlySet<string> {
  const parents = new Map(nodes.map((node) => [node.key, node.parent ?? undefined]));
  const lit = new Set<string>();
  for (const key of base) {
    for (let at: string | undefined = key; at !== undefined && !lit.has(at); at = parents.get(at)) {
      lit.add(at);
    }
  }
  return lit;
}

/**
 * C2: whether the step hides a node: its kind is not drawn, Stages' top-level rule leaves it
 * out, or a container above it is collapsed.
 */
export function stepHides(nodes: readonly GraphNode[], step: Step, collapsed: readonly string[]): (key: string) => boolean {
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  const kinds = new Set(kindsAt(step));
  const out = leftOutAt(step, nodes);
  const shut = new Set(collapsed);
  return (key) => {
    const node = byKey.get(key);
    if (node === undefined || !kinds.has(node.kind) || out.has(key)) {
      return true;
    }
    for (let at = node.parent ?? undefined; at !== undefined; at = byKey.get(at)?.parent ?? undefined) {
      if (shut.has(at)) {
        return true;
      }
    }
    return false;
  };
}

const CLASS_WORDS: Record<Exclude<LevelDisplay, "relevant">, string> = { not_relevant: "not relevant", conditional: "conditional" };

/**
 * C2: what Depth's Fade dims: the nodes the depth would not draw as their own card (`stepHides`
 * over `collapsed`, what Hide at `depth` collapses), among those the level draws (`drawn`, its
 * node keys), less every container above a node the depth does draw: a container holding a node
 * in focus is not dimmed. A container of a node the level does not draw (a filter or a relevance
 * class hides it) holds nothing in focus. Empty with Fade off.
 */
export function depthFaded(state: ViewState, nodes: readonly GraphNode[], depth: Step, collapsed: readonly string[], drawn: ReadonlySet<string>): ReadonlySet<string> {
  if (!state.fade) {
    return new Set();
  }
  const hides = stepHides(nodes, depth, collapsed);
  const parents = new Map(nodes.map((node) => [node.key, node.parent ?? undefined]));
  const faded = new Set([...drawn].filter(hides));
  const cleared = new Set<string>();
  for (const key of drawn) {
    if (!hides(key)) {
      // Walked up from a node in focus, every container above it is cleared (once).
      for (let at = parents.get(key); at !== undefined && !cleared.has(at); at = parents.get(at)) {
        cleared.add(at);
        faded.delete(at);
      }
    }
  }
  return faded;
}

/** DESIGN, Opacity: whether a node is out of focus for Frontier (`lit`, none when Frontier is off) or for Depth (`faded`). Dimmed once, however many say so. */
export function outOfFocus(lit: ReadonlySet<string> | undefined, faded: ReadonlySet<string>): (key: string) => boolean {
  return (key) => faded.has(key) || (lit !== undefined && !lit.has(key));
}

/**
 * C1, C2: the badges on Filter and Depth. Filter counts what the classes switched off hide (the
 * level's count): every class on the graph and the list, only conditional on the timeline (which
 * has no dates for work that does not apply), none on the walk (its cards all apply). Depth counts
 * the nodes Frontier lights (the walk's: its candidate cards) that the depth hides; with Fade on
 * (`fade`) they are drawn, so none.
 */
export function badgesOf(screen: Screen, byClass: Partial<Record<LevelDisplay, number>>, base: ReadonlySet<string> | undefined, hides: (key: string) => boolean, fade = false): { filter?: Badge; depth?: Badge } {
  const classes: (keyof typeof CLASS_WORDS)[] = screen === "walk" ? [] : screen === "timeline" ? ["conditional"] : ["not_relevant", "conditional"];
  const parts = classes.flatMap((each) => {
    const count = byClass[each] ?? 0;
    return count === 0 ? [] : [{ count, words: `${String(count)} ${CLASS_WORDS[each]} hidden` }];
  });
  const lost = base === undefined || fade ? 0 : [...base].filter(hides).length;
  return {
    ...(parts.length === 0 ? {} : { filter: { count: parts.reduce((sum, part) => sum + part.count, 0), words: parts.map((part) => part.words).join(", ") } }),
    ...(lost === 0 ? {} : { depth: { count: lost, words: `${String(lost)} that can move now ${lost === 1 ? "is" : "are"} ${screen === "walk" ? "not walked" : "hidden"} at this depth; choose Actions${screen === "walk" ? "" : " or fade the rest"} to see them` } }),
  };
}

/**
 * C15: what a journey's canvas is laid out for besides its graph: the step it draws at (Actions
 * under Fade, whatever the depth) and what the Filter and search keep, so a change to any lays it
 * out (and fits it) afresh. Frontier and Fade's dimming fade cards and never move one, so they
 * are not part of it; nor are the viewer's expands and collapses, which keep the layout's
 * positions.
 */
export function journeyLayoutView(state: ViewState, depth: Step): string {
  const { filter } = state;
  return JSON.stringify([drawnStep(depth, state.fade), filter.conditional, filter.notRelevant, filter.mine, filter.owner ?? null, FILTER_FLAGS.filter((flag) => filter.flags.includes(flag)), state.text.trim()]);
}
