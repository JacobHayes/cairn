// What the acting surfaces show lives in their address, as the canvas's does (5.2), so a
// filtered list, a re-sorted next list, or a triage mode is shareable, survives a reload, and
// follows the back button. Every setting at its default leaves the address bare. Each screen
// turns its settings into the engine's query (C9 `ListQuery`, C10 `NextQuery`), which the
// derive worker answers over the journey's local derivation.
import type { Schema } from "@cairn/client";
import type { ListQuery, NextQuery } from "@cairn/wasm";

import type { NodeKind, State } from "../detail/model.ts";

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

/** C9, C10: the single signals a list sorts by (Priority: effort when estimates exist). */
export const SORTS: SortBy[] = ["rank", "slack", "gravity", "leverage", "due", "effort"];

/** Every stored state, by kind's machine (D1); a group's is `derived`. */
export const STATES: State[] = ["open", "decided", "todo", "active", "done", "pending", "reached", "skipped", "derived"];

/** Every kind; groups are never on the frontier, so the acting surfaces offer the others. */
export const LIST_KINDS: NodeKind[] = ["decision", "deliverable", "action", "milestone", "group"];
export const ACTING_KINDS: NodeKind[] = ["decision", "deliverable", "action", "milestone"];

/** C9: what the list shows. */
export interface ListSettings {
  flags: ListFlag[];
  /** By group: the container whose subtree the list is narrowed to. */
  within: string | undefined;
  owner: string | undefined;
  states: State[];
  kinds: NodeKind[];
  text: string;
  sort: SortBy;
  /** Grouping by container. */
  grouped: boolean;
}

/** C10: what the next list shows. */
export interface NextSettings {
  sort: SortBy;
  mine: boolean;
  kinds: NodeKind[];
  /** Prioritize for me: rank with the owner factor relative to the viewer (Priority). */
  forMe: boolean;
}

/** C11: what triage shows; `decisions` is the decision walkthrough. */
export interface TriageSettings {
  decisions: boolean;
  mine: boolean;
  kinds: NodeKind[];
}

/** The values of `name` (comma-separated) that are among `known`, in `known`'s order. */
function listed<T extends string>(params: URLSearchParams, name: string, known: readonly T[]): T[] {
  const given = new Set((params.get(name) ?? "").split(","));
  return known.filter((each) => given.has(each));
}

function sortFrom(params: URLSearchParams): SortBy {
  const sort = params.get("sort");
  return SORTS.find((each) => each === sort) ?? "rank";
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
  return {
    flags: listed(params, "flag", LIST_FLAGS),
    within: params.get("in") ?? undefined,
    owner: params.get("owner") ?? undefined,
    states: listed(params, "state", STATES),
    kinds: listed(params, "kind", LIST_KINDS),
    text: params.get("q") ?? "",
    sort: sortFrom(params),
    grouped: params.get("group") === "container",
  };
}

export function listParams(settings: ListSettings): URLSearchParams {
  const params = new URLSearchParams();
  setList(params, "flag", settings.flags);
  if (settings.within !== undefined) {
    params.set("in", settings.within);
  }
  if (settings.owner !== undefined) {
    params.set("owner", settings.owner);
  }
  setList(params, "state", settings.states);
  setList(params, "kind", settings.kinds);
  if (settings.text !== "") {
    params.set("q", settings.text);
  }
  if (settings.sort !== "rank") {
    params.set("sort", settings.sort);
  }
  if (settings.grouped) {
    params.set("group", "container");
  }
  return params;
}

/** C9: the engine's query for `settings`, from `cursor` when paging. */
export function listQueryOf(settings: ListSettings, cursor?: number): ListQuery {
  const query: ListQuery = { flags: settings.flags, states: settings.states, kinds: settings.kinds, sort: settings.sort };
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

export function nextFrom(params: URLSearchParams): NextSettings {
  return {
    sort: sortFrom(params),
    mine: params.get("mine") === "1",
    kinds: listed(params, "kind", ACTING_KINDS),
    forMe: params.get("me") === "1",
  };
}

export function nextParams(settings: NextSettings): URLSearchParams {
  const params = new URLSearchParams();
  if (settings.sort !== "rank") {
    params.set("sort", settings.sort);
  }
  setFlag(params, "mine", settings.mine);
  setList(params, "kind", settings.kinds);
  setFlag(params, "me", settings.forMe);
  return params;
}

/** C10: the engine's query for `settings`. */
export function nextQueryOf(settings: NextSettings): NextQuery {
  return { sort: settings.sort, mine: settings.mine, kinds: settings.kinds, for_viewer: settings.forMe };
}

export function triageFrom(params: URLSearchParams): TriageSettings {
  return {
    decisions: params.get("mode") === "decisions",
    mine: params.get("mine") === "1",
    kinds: listed(params, "kind", ACTING_KINDS),
  };
}

export function triageParams(settings: TriageSettings): URLSearchParams {
  const params = new URLSearchParams();
  if (settings.decisions) {
    params.set("mode", "decisions");
  }
  setFlag(params, "mine", settings.mine);
  if (!settings.decisions) {
    setList(params, "kind", settings.kinds);
  }
  return params;
}

/** C11: triage reads the acting frontier in rank order; the walkthrough only its decisions. */
export function triageQueryOf(settings: TriageSettings): NextQuery {
  return { sort: "rank", mine: settings.mine, kinds: settings.decisions ? ["decision"] : settings.kinds };
}

export const DEFAULT_LIST: ListSettings = listFrom(new URLSearchParams());
export const DEFAULT_NEXT: NextSettings = nextFrom(new URLSearchParams());
export const DEFAULT_TRIAGE: TriageSettings = triageFrom(new URLSearchParams());

export function listPath(journey: string, settings: ListSettings = DEFAULT_LIST): string {
  return `/journeys/${journey}/list${searchOf(listParams(settings))}`;
}

export function nextPath(journey: string, settings: NextSettings = DEFAULT_NEXT): string {
  return `/journeys/${journey}/next${searchOf(nextParams(settings))}`;
}

export function triagePath(journey: string, settings: TriageSettings = DEFAULT_TRIAGE): string {
  return `/journeys/${journey}/triage${searchOf(triageParams(settings))}`;
}

/**
 * C11: the decision walkthrough of `journey`. Starting a journey opens it here (the creation
 * flow, 5.5, navigates to this address once the journey exists), and it can be launched any
 * time from the journey's screens.
 */
export function walkthroughPath(journey: string): string {
  return triagePath(journey, { ...DEFAULT_TRIAGE, decisions: true });
}
