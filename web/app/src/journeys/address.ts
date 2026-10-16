// The addresses of the screens around a journey, and what the journey index shows (C16): its
// filters live in the address, as the canvas's and the acting surfaces' do, so a filtered
// index is shareable and follows the back button. Every filter at its default leaves the
// address bare.
//
// A journey has one page, graph first, with views of its nodes and a walk through its frontier:
//
//   /journeys/<id>/{graph|list|timeline}[/nodes/<key>]   (graph: also /edges/<from>~<to>)
//   /journeys/<id>/summary[/nodes/<key>]
//   /journeys/<id>/walk[/nodes/<key>]
//   /journeys/<id>/nodes/<key>      deep link: the graph with the node selected (JourneyDeepLink)
//   /journeys/<id>                  landing: the view last shown, the graph at first (JourneyLanding)
//
// The view state every view shares is in the query on every view (view.ts: `detail`, `frontier`,
// `decisions`, `mine`, `owner`, `flag`, `show`, `q`), beside the view's own settings.
import type { Schema } from "@cairn/client";

import type { JourneyIndexQuery } from "../data/host.ts";

export type JourneyStatus = Schema<"JourneyStatus">;

/** The statuses, in lifecycle order (B11). */
export const STATUSES: JourneyStatus[] = ["active", "completed", "archived"];

/** C16: the index's filters. `status` is one status, or every status. */
export interface IndexFilters {
  status: JourneyStatus | "any";
  route: string | undefined;
  version: number | undefined;
  /** Only journeys holding something of the caller's ("mine", E4). */
  mine: boolean;
  /** Only journeys whose route has published a newer version (C16, from the host). */
  upgrade: boolean;
}

/**
 * The index opens on active journeys: completed ones leave "mine" and the cross-journey lists
 * (B11), and archived ones are out of the way until asked for.
 */
export const DEFAULT_FILTERS: IndexFilters = { status: "active", route: undefined, version: undefined, mine: false, upgrade: false };

function positive(text: string | null): number | undefined {
  const number = Number(text);
  return text !== null && Number.isInteger(number) && number >= 1 ? number : undefined;
}

export function filtersFrom(params: URLSearchParams): IndexFilters {
  const status = params.get("status");
  const route = params.get("route") ?? undefined;
  return {
    status: status === "any" ? "any" : (STATUSES.find((each) => each === status) ?? "active"),
    route,
    version: route === undefined ? undefined : positive(params.get("version")),
    mine: params.get("mine") === "1",
    upgrade: params.get("upgrade") === "1",
  };
}

export function filterParams(filters: IndexFilters): URLSearchParams {
  const params = new URLSearchParams();
  if (filters.status !== "active") {
    params.set("status", filters.status);
  }
  if (filters.route !== undefined) {
    params.set("route", filters.route);
    if (filters.version !== undefined) {
      params.set("version", String(filters.version));
    }
  }
  if (filters.mine) {
    params.set("mine", "1");
  }
  if (filters.upgrade) {
    params.set("upgrade", "1");
  }
  return params;
}

/**
 * C16: the host's query for `filters`. "Mine" asks the host for the journeys referring to the
 * caller's entities (a superset of those where they hold a participation, E6), active only
 * (B11: a completed journey leaves "mine"); the index then keeps those where the journey's own
 * "mine" is not empty. None when "mine" can hold nothing: the caller has no entity, or the
 * status asked for is not active.
 */
export function queryOf(filters: IndexFilters, entities: readonly string[]): JourneyIndexQuery | undefined {
  const query: JourneyIndexQuery = {};
  if (filters.status !== "any") {
    query.status = [filters.status];
  }
  if (filters.route !== undefined) {
    query.route = filters.route;
    if (filters.version !== undefined) {
      query.version = filters.version;
    }
  }
  if (filters.upgrade) {
    query.upgrade_available = true;
  }
  if (filters.mine) {
    if (entities.length === 0 || (filters.status !== "any" && filters.status !== "active")) {
      return undefined;
    }
    query.status = ["active"];
    query.referencing = [...entities];
  }
  return query;
}

function searchOf(params: URLSearchParams): string {
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

/** C16: the journey index. `/` is the landing, which opens Mine or this (2.1). */
export function indexPath(filters: IndexFilters = DEFAULT_FILTERS): string {
  return `/journeys${searchOf(filterParams(filters))}`;
}

/** C16: the cross-journey "mine" list. */
export const MINE_PATH = "/mine";

/** C16: a journey's landing, where its journey card shows and nothing is selected. */
export function overviewPath(journey: string): string {
  return `/journeys/${journey}`;
}

/** B1: the new-journey form, from a route's version when one is given. */
export function newJourneyPath(route?: string, version?: number): string {
  const params = new URLSearchParams();
  if (route !== undefined) {
    params.set("route", route);
    if (version !== undefined) {
      params.set("version", String(version));
    }
  }
  return `/new${searchOf(params)}`;
}

/** How a journey is drawn: the three projections of its nodes. */
export type Projection = "graph" | "list" | "timeline";

/** The projections in the order the toolbar lists them and `v` cycles them. */
export const PROJECTIONS: readonly Projection[] = ["graph", "list", "timeline"];

/** A journey's screens: a projection, the Summary, or the walk through the frontier. */
export type Screen = Projection | "summary" | "walk";

export const DEFAULT_PROJECTION: Projection = "graph";

/** Key `v`: the projection after `current`, wrapping round. */
export function nextProjection(current: Projection): Projection {
  return PROJECTIONS[(PROJECTIONS.indexOf(current) + 1) % PROJECTIONS.length] ?? DEFAULT_PROJECTION;
}

/** `search` with the parameter `name` set to `value`, or removed when `value` is undefined. */
export function withParam(search: string, name: string, value: string | undefined): string {
  const params = new URLSearchParams(search);
  if (value === undefined) {
    params.delete(name);
  } else {
    params.set(name, value);
  }
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

/** `search` with `?` in front when it holds anything, whichever way it was given. */
function withQuestion(search: string): string {
  return search === "" || search === "?" ? "" : search.startsWith("?") ? search : `?${search}`;
}

/** A journey's screen, or a node's detail on it. `search` is the query, kept as given. */
export function pagePath(journey: string, screen: Screen, node?: string, search = ""): string {
  return `/journeys/${journey}/${screen}${node === undefined ? "" : `/nodes/${node}`}${withQuestion(search)}`;
}

/** The address agents post for a node; it resolves to the graph with the node selected (JourneyDeepLink). */
export function deepLinkPath(journey: string, node: string): string {
  return `/journeys/${journey}/nodes/${node}`;
}
