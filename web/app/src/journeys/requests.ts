// C2, C9, C10, C11, C13: the view state (view.ts) as each view's engine requests and emphasis.
// Hiding is one level request on every view: Depth's kinds and Stages' top-level rule, the
// relevance classes, the Filter and the search, so what is hidden rolls up, its edges re-target,
// and the hidden-prerequisites marker holds wherever it is drawn. The list's table and the walk
// through read their rows beside it and keep the ones the level draws. With Fade on the level is
// the Actions one and Depth dims what it would have hidden. Frontier lights a set of nodes and
// Fade's depth dims another; a node out of focus for either is dimmed, once. Pure, so the unit
// tests read the rules without a page.
import type { ListQuery, ProjectionRequest } from "@cairn/wasm";

import type { ListFlag, SortBy } from "../acting/address.ts";
import { currentStages, drawnStep, kindsAt, leftOutAt, STAGES_TOP } from "../canvas/ladder.ts";
import type { Level, LevelDisplay, Step } from "../canvas/model.ts";
import type { GraphNode, Ready } from "../detail/model.ts";
import type { Badge } from "../screens/badge.ts";
import type { Screen } from "./address.ts";
import { canDraw, FILTER_FLAGS, frontierOn, RELEVANCE_CLASSES, SCREEN_VIEWS, type RelevanceClass, type ViewState } from "./view.ts";

export type LevelRequest = Extract<ProjectionRequest, { projection: "level" }>;

/** The query a level request carries: the request without its tag, as the derive worker's route level takes it. */
export function levelQueryOf(request: LevelRequest): Omit<LevelRequest, "projection"> {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars -- the query is the request less its tag
  const { projection, ...query } = request;
  return query;
}

/** C9: the list flags the Filter asks for: Only mine first, then the flags in the Filter's order. */
function flagsOf(state: ViewState): NonNullable<LevelRequest["flags"]> {
  return [...(state.filter.mine ? (["mine"] as const) : []), ...FILTER_FLAGS.filter((flag) => state.filter.flags.includes(flag))];
}

/**
 * C1, C2, C9: the level every view draws from: the step's kinds (at Stages, only under a top-level
 * stage or milestone), the containers collapsed, the relevance classes shown, and the Filter and
 * search, which keep a node that matches or holds a match. Parts at their default are left out.
 */
export function levelRequestOf(state: ViewState, step: Step, collapsed: readonly string[], layered = false): LevelRequest {
  const { filter } = state;
  // C14: under the proposal layer every display state is drawn, so a change to a conditional or ruled-out node is never hidden by the page's filter.
  const display: LevelDisplay[] = layered ? ["relevant", "conditional", "not_relevant"] : ["relevant", ...(filter.conditional ? (["conditional"] as const) : []), ...(filter.notRelevant ? (["not_relevant"] as const) : [])];
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
 * C2, C7: the level request that draws the selected node: none while `level` (the answer to
 * `request`) draws it and every container above it open, or no node is selected, or the journey
 * has no such node; else `request` naming it `selected`, and the engine draws it with the
 * containers above it whatever hides them. A node can be drawn while a container above it is
 * hoisted past or collapsed, so the whole chain is checked, not the node alone.
 */
export function selectedRequest(request: LevelRequest, level: Pick<Level, "nodes" | "collapsed"> | undefined, selected: string | undefined, nodes: readonly GraphNode[]): LevelRequest | undefined {
  if (selected === undefined || level === undefined) {
    return undefined;
  }
  const byKey = new Map(nodes.map((node) => [node.key, node]));
  if (!byKey.has(selected)) {
    return undefined;
  }
  const drawn = new Set(level.nodes.map((node) => node.key));
  const collapsed = new Set(level.collapsed);
  for (let at: string | undefined = selected, seen = 0; at !== undefined && at !== request.container && seen <= nodes.length; at = byKey.get(at)?.parent ?? undefined, seen += 1) {
    if (!drawn.has(at) || (at !== selected && collapsed.has(at))) {
      return { ...request, selected };
    }
  }
  return undefined;
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

/** C10: the nodes Frontier lights: the acting frontier. None where it is off or lights nothing (view.ts, `SCREEN_VIEWS`). */
export function litOf(screen: Screen, state: ViewState, ready: Ready): ReadonlySet<string> | undefined {
  return frontierOn(screen, state) ? new Set(ready.derived.acting_frontier) : undefined;
}

/** C2, 5.1: the stages that are current in `ready`: those holding an acting-frontier or active node. */
export function currentStagesOf(ready: Ready): Set<string> {
  const nodes = ready.journey.graph.nodes ?? [];
  const active = nodes.filter((node) => ready.derived.nodes[node.key]?.display_state === "active").map((node) => node.key);
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

/**
 * C2, C7: whether the depth leaves a node out of what the level drew (`drawn`, its node keys):
 * `stepHides`, less what the engine's level draws whatever hides it (the selected node and the
 * containers above it). What Depth's badge counts.
 */
export function depthLeavesOut(nodes: readonly GraphNode[], step: Step, collapsed: readonly string[], drawn: ReadonlySet<string> | undefined): (key: string) => boolean {
  const hides = stepHides(nodes, step, collapsed);
  return (key) => hides(key) && drawn?.has(key) !== true;
}

/**
 * C2: what Depth's Fade dims: the nodes the depth would not draw as their own card (`stepHides`
 * over `collapsed`, what Hide at `depth` collapses), among those the level draws (`drawn`, its
 * node keys), less every container above a node the depth does draw: a container holding a node
 * in focus is not dimmed. The selected node is in focus whatever the depth, as the level draws
 * it whatever hides it. A container of a node the level does not draw (a filter or a relevance
 * class hides it) holds nothing in focus. Empty with Fade off.
 */
export function depthFaded(state: ViewState, nodes: readonly GraphNode[], depth: Step, collapsed: readonly string[], drawn: ReadonlySet<string>, selected?: string): ReadonlySet<string> {
  if (!state.fade) {
    return new Set();
  }
  const hides = stepHides(nodes, depth, collapsed);
  const outOfDepth = (key: string) => key !== selected && hides(key);
  const parents = new Map(nodes.map((node) => [node.key, node.parent ?? undefined]));
  const faded = new Set([...drawn].filter(outOfDepth));
  const cleared = new Set<string>();
  for (const key of drawn) {
    if (!outOfDepth(key)) {
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

/** What the Filter hides, counted by the engine's level and shown beside the setting that hides it. */
export interface HiddenCounts {
  /** Per relevance class the view can draw, the nodes only that class hides. */
  classes: Partial<Record<RelevanceClass, number>>;
  /** Per flag set (Only mine included), the nodes that flag alone hides. */
  flags: Partial<Record<ListFlag, number>>;
  /** The nodes the owner setting alone hides. */
  owner?: number;
}

/**
 * C1, C2, C9: what the Filter and Depth say they hide, all from the level's counts and the
 * per-screen table (view.ts, `SCREEN_VIEWS`). `hidden` is what each setting hides, shown beside it
 * in the open Filter; a class the view cannot draw has none, and a view whose cards the level does not read (the walk through) has no flag or owner counts or badge. `filter` is the toolbar's badge: the
 * nodes the Filter's settings hide in all, a ruled-out node never among them (the engine leaves
 * it out of the flag and owner counts; the not-relevant class is left out here). Depth's badge
 * counts the nodes Frontier lights (the walk's: its candidate cards) that the depth hides
 * (`hides`); with Fade on (`fade`) they are drawn, so none.
 */
export function badgesOf(screen: Screen, level: Pick<Level, "hidden_by_class" | "hidden_by_filter"> | undefined, base: ReadonlySet<string> | undefined, hides: (key: string) => boolean, fade = false): { hidden: HiddenCounts; filter?: Badge; depth?: Badge } {
  const classes: Partial<Record<RelevanceClass, number>> = {};
  for (const each of RELEVANCE_CLASSES) {
    const count = level?.hidden_by_class?.[each] ?? 0;
    if (count > 0 && canDraw(screen, each)) {
      classes[each] = count;
    }
  }
  const byFilter = SCREEN_VIEWS[screen].filterCounts ? level?.hidden_by_filter : undefined;
  const flags: Partial<Record<ListFlag, number>> = byFilter?.flags ?? {};
  const hidden: HiddenCounts = { classes, flags, ...(byFilter?.owner === undefined || byFilter.owner === 0 ? {} : { owner: byFilter.owner }) };
  const total = (byFilter?.total ?? 0) + (classes.conditional ?? 0);
  const lost = base === undefined || fade ? 0 : [...base].filter(hides).length;
  return {
    hidden,
    ...(total === 0 ? {} : { filter: { count: total, words: `${String(total)} ${total === 1 ? "node is" : "nodes are"} hidden by the Filter` } }),
    ...(lost === 0 ? {} : { depth: { count: lost, words: `${String(lost)} that can move now ${lost === 1 ? "is" : "are"} ${screen === "walk" ? "not walked" : "hidden"} at this depth; choose Actions${SCREEN_VIEWS[screen].fade ? " or fade the rest" : ""} to see them` } }),
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
