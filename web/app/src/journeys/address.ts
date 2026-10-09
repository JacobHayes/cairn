// The addresses of the screens around a journey, and what the journey index shows (C16): its
// filters live in the address, as the canvas's and the acting surfaces' do, so a filtered
// index is shareable and follows the back button. Every filter at its default leaves the
// address bare.
//
// A journey has two pages, each with projections (2.2):
//
//   /journeys/<id>/next/{list|cards}[/nodes/<key>]
//   /journeys/<id>/plan/{graph|list|timeline}[/nodes/<key>]
//   /journeys/<id>/summary[/nodes/<key>]
//   /journeys/<id>/nodes/<key>      deep link: resolved against the journey (JourneyDeepLink)
//   /journeys/<id>                  landing: resolved against the journey (JourneyLanding)
//
// The toolbar's chips are in the query on every projection: `decisions=1`, `mine=1`, `q=<text>`.
// Every address the earlier screens had still resolves (`legacyRedirect`).
import type { Schema } from "@cairn/client";

import { canvasPath, DEFAULT_VIEW, graphQueryFromOld, OLD_CANVAS_PARAMS } from "../canvas/settings.ts";
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

/** The journey's two pages (2.2). */
export type JourneyPage = "next" | "plan";

/** How a page is drawn. One global order, `GRAPH LIST TIMELINE CARDS`; a page shows its subset. */
export type Projection = "graph" | "list" | "timeline" | "cards";

/** Each page's projections, in the global order. A projection that does not apply is left out. */
export const PAGE_PROJECTIONS: Record<JourneyPage, readonly Projection[]> = {
  next: ["list", "cards"],
  plan: ["graph", "list", "timeline"],
};

export const DEFAULT_PROJECTION: Record<JourneyPage, Projection> = { next: "list", plan: "graph" };

/** `text` as one of `page`'s projections, if it is one. */
export function projectionOf(page: JourneyPage, text: string | undefined): Projection | undefined {
  return PAGE_PROJECTIONS[page].find((each) => each === text);
}

/** Key `v`: the projection after `current` on its page, wrapping round. */
export function nextProjection(page: JourneyPage, current: Projection): Projection {
  const all = PAGE_PROJECTIONS[page];
  const at = all.indexOf(current);
  return all[(at + 1) % all.length] ?? DEFAULT_PROJECTION[page];
}

/**
 * Where each toolbar chip means something: the projections that read its parameter. The
 * graph's search finds a node rather than filtering, so it is not carried.
 */
const CHIP_APPLIES: Record<"decisions" | "mine" | "q", (page: JourneyPage, projection: Projection) => boolean> = {
  decisions: () => true,
  mine: (page, projection) => page === "next" || projection === "list" || projection === "timeline",
  q: (page, projection) => page === "next" || projection !== "graph",
};

/** Whether the chip `name` is a filter on `projection` of `page`. */
export function chipApplies(name: keyof typeof CHIP_APPLIES, page: JourneyPage, projection: Projection): boolean {
  return CHIP_APPLIES[name](page, projection);
}

/**
 * `search` reduced to the toolbar's chips that mean something on `projection` of `page`: what
 * a projection switch or a page switch keeps. A chip a projection does not read is left behind,
 * not carried unseen.
 */
export function carriedSearch(search: string, page: JourneyPage, projection: Projection): string {
  const from = new URLSearchParams(search);
  const kept = new URLSearchParams();
  for (const name of ["decisions", "mine", "q"] as const) {
    const value = from.get(name);
    if (value !== null && chipApplies(name, page, projection)) {
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

/** A page's projection, or a node's detail on it. `search` is the query, kept as given. */
export function pagePath(journey: string, page: JourneyPage, projection: Projection, node?: string, search = ""): string {
  return `/journeys/${journey}/${page}/${projection}${node === undefined ? "" : `/nodes/${node}`}${withQuestion(search)}`;
}

/** C18: the Summary page, the journey card at full width. */
export function summaryPath(journey: string, node?: string): string {
  return `/journeys/${journey}/summary${node === undefined ? "" : `/nodes/${node}`}`;
}

/** The address agents post for a node; it resolves to a page (JourneyDeepLink). */
export function deepLinkPath(journey: string, node: string): string {
  return `/journeys/${journey}/nodes/${node}`;
}

/** The page and projection an address of a journey's page is on, if it is one. */
export function pageOfPath(pathname: string): { page: JourneyPage; projection: Projection } | undefined {
  const match = /^\/journeys\/[^/]+\/(next|plan)\/([a-z]+)(?:\/|$)/.exec(pathname);
  const page = match?.[1] === "next" || match?.[1] === "plan" ? match[1] : undefined;
  const projection = page === undefined ? undefined : projectionOf(page, match?.[2]);
  return page === undefined || projection === undefined ? undefined : { page, projection };
}

/**
 * Where the address agents post for a node resolves (2.4, Deep link): a node on the acting
 * frontier opens on NEXT, LIST with its row selected; any other opens on PLAN, GRAPH,
 * traced, with its container opened, and shown even when it is not relevant. Either way the
 * inspector opens on it.
 */
export function deepLinkTarget(journey: string, node: string, where: { onFrontier: boolean; parent: string | undefined }): string {
  if (where.onFrontier) {
    return pagePath(journey, "next", "list", node);
  }
  return canvasPath(journey, { ...DEFAULT_VIEW, container: where.parent, trace: true }, node);
}

/**
 * C11: a journey with nothing recorded in it yet and a decision to make opens on the
 * walkthrough; any other journey opens on the next list (2.4, landing).
 */
export function landingPath(journey: string, started: { recorded: boolean; decisions: boolean }): string {
  return started.recorded || !started.decisions ? pagePath(journey, "next", "list") : pagePath(journey, "next", "cards", undefined, "?decisions=1");
}

/** The old canvas's parameters, which an address carries only on the canvas of the earlier screens. */
const OLD_CANVAS_ADDRESS = [...OLD_CANVAS_PARAMS, "trace", "edit"];

/** The old screens, each with where it lives now and the query it adds there. */
const OLD_SCREENS: Record<string, { to: [JourneyPage, Projection] | "landing"; add?: string }> = {
  overview: { to: "landing" },
  next: { to: ["next", "list"] },
  list: { to: ["plan", "list"] },
  triage: { to: ["next", "cards"] },
  walkthrough: { to: ["next", "cards"], add: "decisions=1" },
  decisions: { to: ["plan", "graph"], add: "decisions=1" },
  timeline: { to: ["plan", "timeline"] },
};

/**
 * Where an address of the earlier screens lives now (2.4, Redirects), as a path and query, or
 * nothing when `pathname` is not one of them. The old canvas (`/journeys/<id>` or one of its
 * nodes, with the canvas's own query) is PLAN, GRAPH. A bare journey or node address with no
 * canvas query is not old: it is the landing and the deep link, which need the journey to
 * resolve.
 */
export function legacyRedirect(pathname: string, search: string): string | undefined {
  const params = new URLSearchParams(search);
  const screen = /^\/journeys\/([^/]+)\/(overview|next|list|triage|walkthrough|decisions|timeline)(?:\/nodes\/([^/]+))?\/?$/.exec(pathname);
  if (screen !== null) {
    const [, journey = "", name = "", node] = screen;
    const target = OLD_SCREENS[name];
    if (target === undefined) {
      return undefined;
    }
    if (target.to === "landing") {
      return `/journeys/${journey}`;
    }
    const [page, projection] = target.to;
    // The triage's walkthrough was a mode; the chips and settings keep their names.
    const query = new URLSearchParams(projection === "graph" ? graphQueryFromOld(params) : params);
    if (query.get("mode") === "decisions") {
      query.delete("mode");
      query.set("decisions", "1");
    }
    if (target.add !== undefined) {
      const [name_ = "", value = ""] = target.add.split("=");
      query.set(name_, value);
    }
    if (projection === "timeline") {
      // The timeline reads none of the old canvas's settings.
      for (const own of OLD_CANVAS_ADDRESS) {
        query.delete(own);
      }
    }
    return pagePath(journey, page, projection, node, query.toString());
  }
  const canvas = /^\/journeys\/([^/]+)(?:\/nodes\/([^/]+))?\/?$/.exec(pathname);
  if (canvas !== null && OLD_CANVAS_ADDRESS.some((name) => params.has(name))) {
    const [, journey = "", node] = canvas;
    return pagePath(journey, "plan", "graph", node, graphQueryFromOld(params).toString());
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
