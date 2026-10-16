// C2, C9, C10, C11, C13: the view state (view.ts) as each view's engine requests and highlights.
// Hiding is one level request on every view: the ladder's kinds and Stages' top-level rule, the
// relevance classes, the Filter and the search, so what is hidden rolls up, its edges re-target,
// and the hidden-prerequisites marker holds wherever it is drawn. The list's table and the walk
// through read their rows beside it and keep the ones the level draws. Frontier and Decisions
// light a set of nodes; everything else fades (the list and the walk through also narrow to the
// decisions under Decisions). Pure, so the unit tests read the rules without a
// page.
import type { ListQuery, ProjectionRequest } from "@cairn/wasm";

import type { SortBy } from "../acting/address.ts";
import { kindsAt, leftOutAt, STAGES_TOP } from "../canvas/ladder.ts";
import type { LevelDisplay, Step } from "../canvas/model.ts";
import type { GraphNode, Ready } from "../detail/model.ts";
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
 * C9, C13: the list table's read: every row, sorted (by rank when no column is), and only the
 * decisions under Decisions. It carries no filter: the table keeps the rows the level draws, so
 * a search or a filter re-reads only the level.
 */
export function tableQueryOf(state: ViewState, sort: SortBy | undefined, cursor?: number): ListQuery {
  return { sort: sort ?? "rank", ...(state.decisions ? { kinds: ["decision" as const] } : {}), ...(cursor === undefined ? {} : { cursor }) };
}

/**
 * C10, C11: the walk through's cards: the acting frontier in rank order (as the next list ranks
 * it), narrowed by the Filter and the search, only the decisions under Decisions.
 */
export function walkQueryOf(state: ViewState): ListQuery {
  const text = state.text.trim();
  return {
    flags: ["next_up", ...flagsOf(state)],
    sort: "rank",
    ...(state.decisions ? { kinds: ["decision" as const] } : {}),
    ...(state.filter.owner === undefined ? {} : { owner: state.filter.owner }),
    ...(text === "" ? {} : { text }),
  };
}

/**
 * C10, C12: the nodes the highlights light: the acting frontier under Frontier, the decisions
 * under Decisions on the graph and the timeline (the list and the walk show only decisions
 * instead), both at once their intersection. None when nothing is lit, and on the walk through,
 * whose every card is on the frontier.
 */
export function litOf(screen: Screen, state: ViewState, ready: Ready): ReadonlySet<string> | undefined {
  const decisions = state.decisions && (screen === "graph" || screen === "timeline");
  if (screen === "walk" || screen === "summary" || (!state.frontier && !decisions)) {
    return undefined;
  }
  const nodes = ready.journey.graph.nodes ?? [];
  const decided = new Set(nodes.filter((node) => node.kind === "decision").map((node) => node.key));
  const base = state.frontier ? ready.derived.acting_frontier : [...decided];
  return new Set(base.filter((key) => !decisions || decided.has(key)));
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
 * C1, C2: the badges on Filter and Detail. Filter counts what the classes switched off hide (the
 * level's count): every class on the graph and the list, only conditional on the timeline (which
 * has no dates for work that does not apply), none on the walk (its cards all apply). Detail counts
 * the highlighted nodes (the walk's: its candidate cards) the step hides.
 */
export function badgesOf(screen: Screen, byClass: Partial<Record<LevelDisplay, number>>, base: ReadonlySet<string> | undefined, hides: (key: string) => boolean): { filter?: Badge; detail?: Badge } {
  const classes: (keyof typeof CLASS_WORDS)[] = screen === "walk" ? [] : screen === "timeline" ? ["conditional"] : ["not_relevant", "conditional"];
  const parts = classes.flatMap((each) => {
    const count = byClass[each] ?? 0;
    return count === 0 ? [] : [{ count, words: `${String(count)} ${CLASS_WORDS[each]} hidden` }];
  });
  const lost = base === undefined ? 0 : [...base].filter(hides).length;
  return {
    ...(parts.length === 0 ? {} : { filter: { count: parts.reduce((sum, part) => sum + part.count, 0), words: parts.map((part) => part.words).join(", ") } }),
    ...(lost === 0 ? {} : { detail: { count: lost, words: `${String(lost)} highlighted ${lost === 1 ? "is" : "are"} hidden at this detail; choose All to see them` } }),
  };
}

/**
 * C15: what a journey's canvas is laid out for besides its graph: the step and what the Filter
 * and search keep, so a change to either lays it out (and fits it) afresh. The highlights fade
 * cards and never move one, so they are not part of it; nor are the viewer's expands and
 * collapses, which keep the layout's positions.
 */
export function journeyLayoutView(state: ViewState, step: Step): string {
  const { filter } = state;
  return JSON.stringify([step, filter.conditional, filter.notRelevant, filter.mine, filter.owner ?? null, FILTER_FLAGS.filter((flag) => filter.flags.includes(flag)), state.text.trim()]);
}
