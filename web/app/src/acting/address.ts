// What the acting surfaces show lives in their address, as the canvas's does (5.2), so a
// filtered list, a re-sorted list, or a walk through is shareable, survives a reload, and
// follows the back button. Every setting at its default leaves the address bare. Each screen
// turns its settings into the engine's query (C9 `ListQuery`, C10 `NextQuery`), which the
// derive worker answers over the journey's local derivation. The toolbar's chips are shared
// by every view: `decisions=1`, `mine=1`, `q=<text>` and `frontier=1`; the list and the walk
// through live at `/journeys/<id>/list` and `/walk`.
import type { Schema } from "@cairn/client";
import type { ListQuery, NextQuery } from "@cairn/wasm";

import type { NodeKind } from "../detail/model.ts";
import { DISPLAY_STATES, type DisplayState } from "../status/words.ts";

export type ListFlag = Schema<"ListFlag">;
export type SortBy = Schema<"SortBy">;

/** C9's filters, in the order the list offers them. */
export const LIST_FLAGS: ListFlag[] = [
  "mine",
  "unassigned",
  "next_up",
  "decisions_needed",
  "needs_breakdown",
  "active",
  "blocked",
  "overdue",
  "stale",
  "snoozed",
  "snoozed_and_overdue",
  "shortfall",
];

/** The flags the filter offers: the list's by the engine, the walk through's by what its cards carry. The rest still read from an old address. */
export const LIST_FILTER_FLAGS: ListFlag[] = ["overdue", "stale", "unassigned", "shortfall", "snoozed"];
export const WALK_FILTER_FLAGS: ListFlag[] = LIST_FILTER_FLAGS.filter((flag) => flag !== "snoozed");

/** C9, C10: the single signals a list sorts by (Priority: effort when estimates exist). */
export const SORTS: SortBy[] = ["rank", "slack", "gravity", "unlocks", "due", "effort"];

/** D8: the states the list's filter offers: every display state but not relevant, which has its own switch. */
export const STATES: DisplayState[] = DISPLAY_STATES.filter((state) => state !== "not_relevant");

/** C9: the columns Columns adds beyond the ones every list shows, in the order they sit. */
export const LIST_COLUMNS = ["rank", "start_by", "slack", "gravity", "unlocks", "kind", "answer", "why", "affects", "effort"] as const;
export type ListColumn = (typeof LIST_COLUMNS)[number];

/** Every kind; groups are never on the frontier, so the acting surfaces offer the others. */
export const LIST_KINDS: NodeKind[] = ["decision", "deliverable", "action", "milestone", "group"];
export const ACTING_KINDS: NodeKind[] = ["decision", "deliverable", "action", "milestone"];

/** C9: what the list shows. */
export interface ListSettings {
  flags: ListFlag[];
  /** By group: the container whose subtree the list is narrowed to. */
  within: string | undefined;
  owner: string | undefined;
  /** The display states shown; any (not relevant apart) when empty. */
  states: DisplayState[];
  /** Not-relevant rows are hidden unless asked for (C9, 5.2). */
  notRelevant: boolean;
  kinds: NodeKind[];
  text: string;
  /** The one signal the rows are sorted by, which flattens the tree; none keeps plan order. */
  sort: SortBy | undefined;
  /** The columns added to the defaults. */
  columns: ListColumn[];
  /** The DECISIONS chip: the decisions only (C12). */
  decisions: boolean;
  /** The Frontier chip: only what can move now, flat, in rank order (C10). */
  frontier: boolean;
}

/** C11: what the walk through shows; `decisions` is the decision walkthrough. */
export interface TriageSettings {
  decisions: boolean;
  mine: boolean;
  kinds: NodeKind[];
  /** The cards kept: those with every one of these flags (`WALK_FILTER_FLAGS`). */
  flags: ListFlag[];
  /** The search: cards whose title contains it. */
  text: string;
}

/** The values of `name` (comma-separated) that are among `known`, in `known`'s order. */
function listed<T extends string>(params: URLSearchParams, name: string, known: readonly T[]): T[] {
  const given = new Set((params.get(name) ?? "").split(","));
  return known.filter((each) => given.has(each));
}

function listSortFrom(params: URLSearchParams): SortBy | undefined {
  const sort = params.get("sort");
  return SORTS.find((each) => each === sort);
}

function setList(params: URLSearchParams, name: string, values: readonly string[]): void {
  if (values.length > 0) {
    params.set(name, values.join(","));
  }
}

function setFlag(params: URLSearchParams, name: string, on: boolean): void {
  if (on) {
    params.set(name, "1");
  }
}

function searchOf(params: URLSearchParams): string {
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

export function listFrom(params: URLSearchParams): ListSettings {
  const flags = new Set(listed(params, "flag", LIST_FLAGS));
  if (params.get("mine") === "1") {
    flags.add("mine");
  }
  return {
    flags: LIST_FLAGS.filter((flag) => flags.has(flag)),
    within: params.get("in") ?? undefined,
    owner: params.get("owner") ?? undefined,
    states: listed(params, "state", STATES),
    notRelevant: params.get("show") === "notrelevant",
    kinds: listed(params, "kind", LIST_KINDS),
    text: params.get("q") ?? "",
    sort: listSortFrom(params),
    columns: listed(params, "cols", LIST_COLUMNS),
    decisions: params.get("decisions") === "1",
    frontier: params.get("frontier") === "1",
  };
}

export function listParams(settings: ListSettings): URLSearchParams {
  const params = new URLSearchParams();
  setFlag(params, "decisions", settings.decisions);
  setFlag(params, "frontier", settings.frontier);
  setFlag(params, "mine", settings.flags.includes("mine"));
  setList(params, "flag", settings.flags.filter((flag) => flag !== "mine"));
  if (settings.within !== undefined) {
    params.set("in", settings.within);
  }
  if (settings.owner !== undefined) {
    params.set("owner", settings.owner);
  }
  setList(params, "state", settings.states);
  if (settings.notRelevant) {
    params.set("show", "notrelevant");
  }
  setList(params, "kind", settings.kinds);
  if (settings.text !== "") {
    params.set("q", settings.text);
  }
  if (settings.sort !== undefined) {
    params.set("sort", settings.sort);
  }
  setList(params, "cols", settings.columns);
  return params;
}

/**
 * C9, D8: the display states a list asks for. Not-relevant rows are left out unless asked for,
 * so the list holds what the plan's count does; states chosen by name are all there is.
 */
export function listDisplayStates(settings: ListSettings): DisplayState[] {
  if (settings.states.length > 0) {
    return settings.notRelevant ? [...settings.states, "not_relevant"] : settings.states;
  }
  return settings.notRelevant ? [] : STATES;
}

/** C9: the engine's query for `settings`, from `cursor` when paging. */
export function listQueryOf(settings: ListSettings, cursor?: number): ListQuery {
  const flags = settings.frontier && !settings.flags.includes("next_up") ? [...settings.flags, "next_up" as const] : settings.flags;
  const query: ListQuery = { flags, display_states: listDisplayStates(settings), kinds: settings.decisions ? ["decision"] : settings.kinds, sort: settings.sort ?? "rank" };
  if (settings.within !== undefined) {
    query.within = settings.within;
  }
  if (settings.owner !== undefined) {
    query.owner = settings.owner;
  }
  if (settings.text.trim() !== "") {
    query.text = settings.text.trim();
  }
  if (cursor !== undefined) {
    query.cursor = cursor;
  }
  return query;
}

export function triageFrom(params: URLSearchParams): TriageSettings {
  return {
    decisions: params.get("decisions") === "1",
    mine: params.get("mine") === "1",
    kinds: listed(params, "kind", ACTING_KINDS),
    flags: listed(params, "flag", WALK_FILTER_FLAGS),
    text: params.get("q") ?? "",
  };
}

export function triageParams(settings: TriageSettings): URLSearchParams {
  const params = new URLSearchParams();
  setFlag(params, "decisions", settings.decisions);
  setFlag(params, "mine", settings.mine);
  if (!settings.decisions) {
    setList(params, "kind", settings.kinds);
  }
  setList(params, "flag", settings.flags);
  if (settings.text !== "") {
    params.set("q", settings.text);
  }
  return params;
}

/** C11: triage reads the acting frontier in rank order; the walkthrough only its decisions. */
export function triageQueryOf(settings: TriageSettings): NextQuery {
  return { sort: "rank", mine: settings.mine, kinds: settings.decisions ? ["decision"] : settings.kinds };
}

export const DEFAULT_LIST: ListSettings = listFrom(new URLSearchParams());
export const DEFAULT_TRIAGE: TriageSettings = triageFrom(new URLSearchParams());

/** A node's detail on a screen at `path`: its `/nodes/<key>` is added before the query. */
function withNode(path: string, node: string | undefined): string {
  return node === undefined ? path : `${path}/nodes/${node}`;
}

export function listPath(journey: string, settings: ListSettings = DEFAULT_LIST, node?: string): string {
  return `${withNode(`/journeys/${journey}/list`, node)}${searchOf(listParams(settings))}`;
}

export function triagePath(journey: string, settings: TriageSettings = DEFAULT_TRIAGE, node?: string): string {
  return `${withNode(`/journeys/${journey}/walk`, node)}${searchOf(triageParams(settings))}`;
}

/**
 * C11: the decision walkthrough of `journey`, the walk through with DECISIONS on. Starting a journey
 * opens it here (the creation flow, 5.5, navigates to this address once the journey exists),
 * and it can be launched any time from the journey's pages.
 */
export function walkthroughPath(journey: string): string {
  return triagePath(journey, { ...DEFAULT_TRIAGE, decisions: true });
}
