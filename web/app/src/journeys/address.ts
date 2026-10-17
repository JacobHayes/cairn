// The addresses of the journey pages, the index and Home (C16): what an address names is in its
// path, how it is looked at in its query, so a page is shareable, survives a reload and follows
// the back button. The path mirrors the API:
//
//   /                                       Home: what waits on the viewer, then their journeys
//   /journeys?status=&route=&version=&mine=&upgrade=   the journey index; its filters in the query
//   /journeys/new?route=&version=           the new-journey form
//   /journeys/<id>                          a journey
//   /journeys/<id>/nodes/<key>              a node selected in it
//   /journeys/<id>/edges/<from>~<to>        an edge's card (the graph)
//
// `view=graph|list|timeline|summary|walk` says how a journey is looked at, and is written only
// when it differs from the address's default (`defaultScreen`): the Summary for a bare journey,
// the Graph for a node or an edge, so one state has one address. Beside it the view state every view
// shares (view.ts: `depth`, `fade`, `frontier`, `mine`, `owner`, `flag`, `show`, `q`) and each view's
// own settings (the graph's `open`, `shut`, `lens`, `edit`, `map`; the list's `sort`, `cols`; the
// timeline's `range`), which stay in the query when the view changes, so switching back restores
// them. Nothing redirects, and no earlier address is kept.
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

/** Home (`/`): what waits on the viewer, then their active journeys. */
export const HOME_PATH = "/";

/** C16: the journey index, `/journeys`, its filters in the query. */
export function indexPath(filters: IndexFilters = DEFAULT_FILTERS): string {
  return `/journeys${searchOf(filterParams(filters))}`;
}

/** B1: the new-journey form, `/journeys/new`, from a route's version when one is given. */
export function newJourneyPath(route?: string, version?: number): string {
  const params = new URLSearchParams();
  if (route !== undefined) {
    params.set("route", route);
    if (version !== undefined) {
      params.set("version", String(version));
    }
  }
  return `/journeys/new${searchOf(params)}`;
}

/** How a journey is drawn: the three projections of its nodes. */
export type Projection = "graph" | "list" | "timeline";

/** The projections in the order the View picker lists them and `v` cycles them. */
export const PROJECTIONS: readonly Projection[] = ["graph", "list", "timeline"];

/** A journey's screens: a projection, the Summary, or the Walkthrough of the frontier. */
export type Screen = Projection | "summary" | "walk";

/** C11: the screens in the order the View picker lists them: the Walkthrough is one more view. */
export const SCREENS: readonly Screen[] = ["graph", "list", "timeline", "summary", "walk"];

/** Each screen's name as the View picker says it. */
export const SCREEN_WORDS: Record<Screen, string> = { graph: "Graph", list: "List", timeline: "Timeline", summary: "Summary", walk: "Walkthrough" };

/** Key `v`: the projection after `current`, wrapping round. */
export function nextProjection(current: Projection): Projection {
  return PROJECTIONS[(PROJECTIONS.indexOf(current) + 1) % PROJECTIONS.length] ?? "graph";
}

/** `search` with the parameter `name` set to `value`, or removed when `value` is undefined. */
export function withParam(search: string, name: string, value: string | undefined): string {
  const params = new URLSearchParams(search);
  if (value === undefined) {
    params.delete(name);
  } else {
    params.set(name, value);
  }
  return searchOf(params);
}

/** What a journey address names: the journey, and a node or an edge (`<from>~<to>`) in it. */
export interface JourneyTarget {
  journey: string;
  node?: string | undefined;
  edge?: string | undefined;
}

/**
 * The view an address opens when its query names none: the Summary for a bare journey, the Graph
 * for a node or an edge (a node's place and neighbourhood is what a node link is for). The one
 * place that says so.
 */
export function defaultScreen(target: JourneyTarget): Screen {
  return target.node === undefined && target.edge === undefined ? "summary" : "graph";
}

/** The view `params` names for `target` (`view=`), or its default; an unknown value is the default. */
export function screenOf(target: JourneyTarget, params: URLSearchParams): Screen {
  const view = params.get("view");
  return SCREENS.find((each) => each === view) ?? defaultScreen(target);
}

/** The path of `target` alone: `/journeys/<id>[/nodes/<key> | /edges/<from>~<to>]`. */
function targetPath(target: JourneyTarget): string {
  const place = target.node !== undefined ? `/nodes/${target.node}` : target.edge !== undefined ? `/edges/${target.edge}` : "";
  return `/journeys/${target.journey}${place}`;
}

/**
 * `target` showing `screen`: `search` kept as given (with or without its `?`), but for `view`,
 * written first and only when `screen` is not `defaultScreen(target)`.
 */
export function journeyPath(target: JourneyTarget, screen: Screen = defaultScreen(target), search = ""): string {
  const params = new URLSearchParams(search);
  params.delete("view");
  const query = new URLSearchParams();
  if (screen !== defaultScreen(target)) {
    query.set("view", screen);
  }
  for (const [name, value] of params) {
    query.append(name, value);
  }
  return `${targetPath(target)}${searchOf(query)}`;
}

/**
 * The journey address `pathname` is, or none for any other page (`/journeys/new` is not one). Only
 * `nodeOpened` reads a path, since it serves pages that are not a journey's; a component on a
 * journey page takes its target from the router (`useJourneyTarget`).
 */
function journeyTargetOf(pathname: string): JourneyTarget | undefined {
  const found = /^\/journeys\/([^/]+)(?:\/nodes\/([^/]+)|\/edges\/([^/]+))?$/.exec(pathname);
  if (found === null || found[1] === undefined || (found[1] === "new" && found[2] === undefined && found[3] === undefined)) {
    return undefined;
  }
  return { journey: found[1], node: found[2], edge: found[3] };
}

/**
 * From the page at `pathname` + `search`, the same page with node `key` open, or none when `key`
 * is undefined; an open edge closes either way. On a journey page the view stays the one shown
 * (the address's default can change with the node, the view does not). On another page with a
 * detail column (a route's draft, a proposal) `/nodes/<key>` or `/edges/...` is taken off its path
 * and `/nodes/<key>` added.
 */
export function nodeOpened(location: string, search: string, key: string | undefined): string {
  const pathname = location.length > 1 ? location.replace(/\/+$/, "") : location;
  const target = journeyTargetOf(pathname);
  if (target !== undefined) {
    const screen = screenOf(target, new URLSearchParams(search));
    return journeyPath({ journey: target.journey, node: key }, screen, search);
  }
  const page = pathname.replace(/\/(?:nodes|edges)\/[^/]*$/, "");
  return `${key === undefined ? page : `${page}/nodes/${key}`}${withQuestion(search)}`;
}

/**
 * View `to` from the journey page at `target` + `search`: the whole query but `map` (the phone's
 * full-screen map is a moment of the graph), so the other views' own settings stay for the way
 * back. The node stays (or becomes `node` when given: Show in list, Show in graph, the Affects
 * link), except into the Walkthrough from another view: there a node's address means "opened from
 * a card", and a node opened elsewhere is no card of this pass. An edge stays only in the graph.
 */
export function screenSwitched(target: JourneyTarget, search: string, to: Screen, node?: string): string {
  const from = screenOf(target, new URLSearchParams(search));
  const params = new URLSearchParams(search);
  if (from !== to) {
    params.delete("map");
  }
  const keeps = node ?? (to === "walk" && from !== "walk" ? undefined : target.node);
  const edge = to === "graph" && node === undefined && keeps === undefined ? target.edge : undefined;
  return journeyPath({ journey: target.journey, node: keeps, edge }, to, params.toString());
}

/**
 * C11: the decision walkthrough a new journey opens on: the walk through at Decisions depth
 * (the creation flow, 5.5, navigates here once the journey exists; it can be launched any time).
 */
export function walkthroughPath(journey: string): string {
  return journeyPath({ journey }, "walk", "?depth=decisions");
}

/** `search` with `?` in front when it holds anything, whichever way it was given. */
function withQuestion(search: string): string {
  return search === "" || search === "?" ? "" : search.startsWith("?") ? search : `?${search}`;
}
