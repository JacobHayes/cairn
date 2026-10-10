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
// The toolbar's chips are in the query on every view: `decisions=1`, `mine=1`, `q=<text>`, and
// `frontier=1`, the lens on what can move now. Every address the earlier screens had still
// resolves (`legacyRedirect`).
import type { Schema } from "@cairn/client";

import { graphQueryFromOld, OLD_CANVAS_PARAMS } from "../canvas/settings.ts";
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

/** The journey index's parameters (filterParams). */
const INDEX_PARAMS = ["status", "route", "version", "mine", "upgrade"];

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

/** `text` as a projection, if it is one. */
export function projectionOf(text: string | undefined): Projection | undefined {
  return PROJECTIONS.find((each) => each === text);
}

/** Key `v`: the projection after `current`, wrapping round. */
export function nextProjection(current: Projection): Projection {
  return PROJECTIONS[(PROJECTIONS.indexOf(current) + 1) % PROJECTIONS.length] ?? DEFAULT_PROJECTION;
}

/**
 * Where each toolbar chip means something: the screens that read its parameter. The graph's
 * search finds a node rather than filtering, so it is not carried; the walk through is the
 * frontier already.
 */
const CHIP_APPLIES: Record<"decisions" | "mine" | "q" | "frontier", (screen: Screen) => boolean> = {
  decisions: (screen) => screen !== "summary",
  mine: (screen) => screen !== "summary" && screen !== "graph",
  q: (screen) => screen !== "summary" && screen !== "graph",
  frontier: (screen) => screen === "graph" || screen === "list" || screen === "timeline",
};

/** Whether the chip `name` is a filter on `screen`. */
export function chipApplies(name: keyof typeof CHIP_APPLIES, screen: Screen): boolean {
  return CHIP_APPLIES[name](screen);
}

/**
 * `search` reduced to the toolbar's chips that mean something on `screen`: what a switch of
 * view keeps. A chip a screen does not read is left behind, not carried unseen.
 */
export function carriedSearch(search: string, screen: Screen): string {
  const from = new URLSearchParams(search);
  const kept = new URLSearchParams();
  for (const name of ["decisions", "mine", "q", "frontier"] as const) {
    const value = from.get(name);
    if (value !== null && chipApplies(name, screen)) {
      kept.set(name, value);
    }
  }
  return searchOf(kept);
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

/**
 * `search` with MINE flipped as the list and the cards read it: on when `mine=1` or the list's own
 * `flag=mine` says so, which flipping off takes out of both.
 */
export function withMineFlipped(search: string): string {
  const params = new URLSearchParams(search);
  const flags = (params.get("flag") ?? "").split(",").filter(Boolean);
  if (params.get("mine") !== "1" && !flags.includes("mine")) {
    return withParam(search, "mine", "1");
  }
  params.delete("mine");
  const rest = flags.filter((flag) => flag !== "mine");
  if (rest.length > 0) {
    params.set("flag", rest.join(","));
  } else {
    params.delete("flag");
  }
  return searchOf(params);
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

/** The old canvas's parameters, which an address carries only on the canvas of the earlier screens. */
const OLD_CANVAS_ADDRESS = [...OLD_CANVAS_PARAMS, "trace", "edit"];

/** Where an address that no longer exists lives now, and the query it adds there. */
interface Moved {
  to: Screen;
  add?: string;
}

/** The earlier screens at `/journeys/<id>/<name>`. */
const OLD_SCREENS: Record<string, Moved | "landing"> = {
  overview: "landing",
  next: { to: "list", add: "frontier=1" },
  triage: { to: "walk" },
  walkthrough: { to: "walk", add: "decisions=1" },
  decisions: { to: "graph", add: "decisions=1" },
};

/** The two pages that held the projections before the journey had one: `next/{list|cards}` and `plan/{graph|list|timeline}`. */
const OLD_PAGES: Record<string, Record<string, Moved>> = {
  next: { list: { to: "list", add: "frontier=1" }, cards: { to: "walk" } },
  plan: { graph: { to: "graph" }, list: { to: "list" }, timeline: { to: "timeline" } },
};

/** `moved`'s address for `journey` and `node`, with `params` carried over (the old canvas's and the triage's parameters under their new names). */
function movedTo(journey: string, moved: Moved, node: string | undefined, params: URLSearchParams, of: "nodes" | "edges" = "nodes"): string {
  const query = new URLSearchParams(moved.to === "graph" ? graphQueryFromOld(params) : params);
  // The triage's walkthrough was a mode; the chips and settings keep their names.
  if (query.get("mode") === "decisions") {
    query.delete("mode");
    query.set("decisions", "1");
  }
  if (moved.add !== undefined) {
    const [name = "", value = ""] = moved.add.split("=");
    query.set(name, value);
  }
  if (moved.to === "timeline") {
    // The timeline reads none of the old canvas's settings.
    for (const own of OLD_CANVAS_ADDRESS) {
      query.delete(own);
    }
  }
  const path = pagePath(journey, moved.to, node, query.toString());
  return of === "edges" ? path.replace("/nodes/", "/edges/") : path;
}

/**
 * Where an address that no longer exists lives now, as a path and query, or nothing when
 * `pathname` is none of them: the earlier screens, the two pages, the old canvas (`/journeys/<id>`
 * or one of its nodes with the canvas's own query), and a `/nodes/` or `/edges/` with no key,
 * which is the view without it. A bare journey or node address with no canvas query is not old:
 * it is the landing and the deep link.
 */
export function legacyRedirect(pathname: string, search: string): string | undefined {
  const params = new URLSearchParams(search);
  const keyless = /^(\/journeys\/[^/]+(?:\/(?:next|plan|graph|list|timeline|summary|walk)(?:\/[a-z]+)?)?)\/(?:nodes|edges)\/?$/.exec(pathname);
  if (keyless !== null) {
    return `${keyless[1] ?? ""}${withQuestion(search)}`;
  }
  const paged = /^\/journeys\/([^/]+)\/(next|plan)(?:\/([a-z]+))?(?:\/(nodes|edges)\/([^/]+))?\/?$/.exec(pathname);
  if (paged !== null) {
    const [, journey = "", page = "", projection, of, node] = paged;
    const moved = OLD_PAGES[page]?.[projection ?? (page === "plan" ? "graph" : "list")];
    return moved === undefined ? undefined : movedTo(journey, moved, node, params, of === "edges" ? "edges" : "nodes");
  }
  const screen = /^\/journeys\/([^/]+)\/(overview|next|triage|walkthrough|decisions)(?:\/nodes\/([^/]+))?\/?$/.exec(pathname);
  if (screen !== null) {
    const [, journey = "", name = "", node] = screen;
    const moved = OLD_SCREENS[name];
    if (moved === undefined) {
      return undefined;
    }
    return moved === "landing" ? `/journeys/${journey}` : movedTo(journey, moved, node, params);
  }
  const canvas = /^\/journeys\/([^/]+)(?:\/nodes\/([^/]+))?\/?$/.exec(pathname);
  if (canvas !== null && OLD_CANVAS_ADDRESS.some((name) => params.has(name))) {
    const [, journey = "", node] = canvas;
    return movedTo(journey, { to: "graph" }, node, params);
  }
  if (/^\/routes\/?$/.test(pathname)) {
    return "/library?type=routes";
  }
  // The index was at `/` with its filters; `/` is the landing now.
  if (pathname === "/" && INDEX_PARAMS.some((name) => params.has(name))) {
    return `${indexPath()}${searchOf(params)}`;
  }
  return undefined;
}
