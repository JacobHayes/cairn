// The addresses of the journey pages, the index and Home (C16): what an address names is in its
// path, how it is looked at in its query, so a page is shareable, survives a reload and follows
// the back button. The path mirrors the API:
//
//   /                                       Home: what waits on the viewer, then their journeys
//   /journeys?status=&route=&version=&mine=&upgrade=   the journey index; its filters in the query
//   /journeys/new?route=&version=           the new-journey form
//   /journeys/<id>                          a journey
//   /journeys/<id>/nodes/<key>              a node selected in it
//   /journeys/<id>/edges/<from>~<to>        a drawn line's card: the line between those two cards at the view the query describes (always the Graph)
//   /journeys/<id>/proposals/<pid>[/nodes/<key>]   the journey with a proposal's layer on (C14)
//
// `view=graph|list|timeline|summary|walk` says how a journey is looked at, and is written only
// when it differs from the address's default (`defaultScreen`): the Summary for a bare journey,
// the Graph for a node or a proposal's layer, and always for an edge, so one state has one address. Beside it the view state every view
// shares (view.ts: `depth`, `fade`, `frontier`, `mine`, `owner`, `flag`, `show`, `q`) and each view's
// own settings (the graph's `open`, `shut`, `lens`, `edit`; the list's `sort`, `cols`; the
// timeline's `range`), which stay in the query when the view changes, so switching back restores
// them. Nothing redirects, and no earlier address is kept.
import { useMemo } from "react";
import { useLocation } from "react-router";

import { JOURNEY_STATUSES, STATUS_HOLDS_CURRENT_WORK, type Schema } from "@cairn/client";

import { pathTo, proposalAddress, required, usePage, type Page } from "../address.ts";
import type { JourneyIndexQuery } from "../data/host.ts";
import { routePath, routeScreenOf, spotOf } from "../routes/address.ts";

export type JourneyStatus = Schema<"JourneyStatus">;

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
    status: status === "any" ? "any" : (JOURNEY_STATUSES.find((each) => each === status) ?? "active"),
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
 * caller's identities (a superset of those where they hold a participation, E6), in the statuses
 * that hold current work (B11: a completed journey leaves "mine"); the index then keeps those
 * where the journey's own "mine" is not empty. None when "mine" can hold nothing: the caller has
 * no identity, or the status asked for holds no current work.
 */
export function queryOf(filters: IndexFilters, identities: readonly string[]): JourneyIndexQuery | undefined {
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
    const current = JOURNEY_STATUSES.filter((status) => STATUS_HOLDS_CURRENT_WORK[status] && (filters.status === "any" || filters.status === status));
    if (identities.length === 0 || current.length === 0) {
      return undefined;
    }
    query.status = current;
    query.referencing = [...identities];
  }
  return query;
}

function searchOf(params: URLSearchParams): string {
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

/** Home (`/`): what waits on the viewer, then their active journeys. */
export const HOME_PATH = pathTo("home");

/** C16: the journey index, `/journeys`, its filters in the query. */
export function indexPath(filters: IndexFilters = DEFAULT_FILTERS): string {
  return `${pathTo("journeys")}${searchOf(filterParams(filters))}`;
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
  return `${pathTo("journeyNew")}${searchOf(params)}`;
}

/** How a journey is drawn: the three projections of its nodes. */
export type Projection = "graph" | "list" | "timeline";

/** The projections in the order the View picker lists them and `v` cycles them. */
export const PROJECTIONS: readonly Projection[] = ["graph", "list", "timeline"];

/** A journey's screens: a projection, the Summary, or the Walkthrough of the frontier. */
export type Screen = Projection | "summary" | "walk";

/** Each screen's name as the View picker says it. */
export const SCREEN_WORDS: Record<Screen, string> = { graph: "Graph", list: "List", timeline: "Timeline", summary: "Summary", walk: "Walkthrough" };

const SCREEN_NAMES = Object.keys(SCREEN_WORDS) as Screen[];

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
  /** The proposal whose layer is on (C14); the page is the journey's own, with the proposal drawn over it. */
  proposal?: string | undefined;
}

/**
 * The view an address opens when its query names none: the Summary for a bare journey, the Graph
 * for a node, an edge or a proposal's layer (a node's place and neighbourhood is what a node link
 * is for, and the layer has the Summary and the Walkthrough nothing to say). The one place that
 * says so.
 */
export function defaultScreen(target: JourneyTarget): Screen {
  return target.node === undefined && target.edge === undefined && target.proposal === undefined ? "summary" : "graph";
}

/** C14: the views a proposal's layer is not drawn over, each with why: the Summary and the Walkthrough describe the journey as it is. */
const NOT_UNDER_LAYER: Partial<Record<Screen, string>> = { summary: "Close the proposal to see the summary", walk: "Close the proposal to walk through the journey" };

/** The view a proposal's layer draws for `screen`: the Graph in place of a view it is not drawn over. */
export function layerScreen(screen: Screen): Screen {
  return NOT_UNDER_LAYER[screen] === undefined ? screen : "graph";
}

/**
 * C11, C14: the views a journey's page at `target` lists but cannot show now, each with why:
 * under a proposal's layer, in any mode, the Summary and the Walkthrough. The View picker shows
 * them disabled with the reason, and their keys do nothing.
 */
export function unavailableScreens(target: JourneyTarget): Partial<Record<Screen, string>> {
  return target.proposal === undefined ? {} : NOT_UNDER_LAYER;
}

/**
 * The view `params` names for `target` (`view=`), or its default; an unknown value is the default.
 * A drawn line is the Graph's, whatever `view=` says.
 * Under a proposal's layer the Summary and the Walkthrough resolve to the Graph.
 */
export function screenOf(target: JourneyTarget, params: URLSearchParams): Screen {
  const view = params.get("view");
  const named = target.edge !== undefined ? "graph" : (SCREEN_NAMES.find((each) => each === view) ?? defaultScreen(target));
  return target.proposal === undefined ? named : layerScreen(named);
}

/** The path of `target` alone: a journey, a node in it, an edge (`<from>~<to>`), or a proposal's layer over either. */
function targetPath(target: JourneyTarget): string {
  if (target.proposal !== undefined) {
    const [journey, proposal] = [target.journey, target.proposal];
    return target.node === undefined ? pathTo("journeyProposal", { journey, proposal }) : pathTo("journeyProposalNode", { journey, proposal, key: target.node });
  }
  if (target.node !== undefined) {
    return pathTo("journeyNode", { journey: target.journey, key: target.node });
  }
  return target.edge !== undefined ? pathTo("journeyEdge", { journey: target.journey, edge: target.edge }) : pathTo("journey", { journey: target.journey });
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
 * From `page` + `search`, the same page with node `key` open, or none when `key` is undefined; an
 * open edge closes either way. On a journey page the view stays the one shown (the address's
 * default can change with the node, the view does not). A route's node (the draft's or a
 * version's) and a proposal's review (under any destination) take the node in their own path;
 * closing keeps the version. A journey's or route's open proposals
 * have no node to close; a node opened from them opens on the journey, or in the route's
 * draft. Any other page, the deployment's open proposals among them, is unchanged.
 * A proposal's layer is the page's own with the proposal on, so a node picked or closed keeps it.
 */
export function nodeAddress(page: Page, search: string, key: string | undefined): string {
  const { name, params } = page;
  const query = withQuestion(search);
  switch (name) {
    case "journey":
    case "journeyNode":
    case "journeyEdge": {
      const target = { journey: required(params, "journey"), node: params.key, edge: params.edge };
      return journeyPath({ journey: target.journey, node: key }, screenOf(target, new URLSearchParams(search)), search);
    }
    case "journeyProposals": {
      const journey = required(params, "journey");
      return key === undefined ? `${pathTo("journeyProposals", { journey })}${query}` : journeyPath({ journey, node: key }, "graph", search);
    }
    case "route":
    case "routeNode": {
      const route = required(params, "route");
      return routePath(route, { ...spotOf(new URLSearchParams(search)), node: key }, search);
    }
    case "routeEdge": {
      // A line is the Graph's, so a node opened from it is opened there.
      const route = required(params, "route");
      return routePath(route, { version: spotOf(new URLSearchParams(search)).version, node: key, screen: "graph" }, search);
    }
    case "routeProposals": {
      const route = required(params, "route");
      return key === undefined ? `${pathTo("routeProposals", { route })}${query}` : routePath(route, { node: key, screen: routeScreenOf(new URLSearchParams(search)) }, search);
    }
    case "journeyProposal":
    case "journeyProposalNode": {
      const target = { journey: required(params, "journey"), proposal: required(params, "proposal") };
      return journeyPath({ ...target, node: key }, screenOf(target, new URLSearchParams(search)), search);
    }
    case "routeProposal":
    case "routeProposalNode":
      return routePath(required(params, "route"), { proposal: required(params, "proposal"), node: key, screen: routeScreenOf(new URLSearchParams(search)) }, search);
    case "identity":
    case "newIdentity":
      return `${pathTo("identities")}${query}`;
    case "deploymentProposal":
      return `${proposalAddress("deployment", required(params, "proposal"))}${query}`;
    default:
      return `${pathTo(name)}${query}`;
  }
}

/**
 * From `page` + `search`, the address of the line drawn from card `from` to card `to`: on the
 * journey's or the route's Graph, the view settings and the version kept, no node open. Any other
 * page is unchanged.
 */
export function edgeAddress(page: Page, search: string, from: string, to: string): string {
  const { name, params } = page;
  const edge = `${from}~${to}`;
  switch (name) {
    case "journey":
    case "journeyNode":
    case "journeyEdge":
      return journeyPath({ journey: required(params, "journey"), edge }, "graph", search);
    case "route":
    case "routeNode":
    case "routeEdge":
      return routePath(required(params, "route"), { version: spotOf(new URLSearchParams(search)).version, edge, screen: "graph" }, search);
    default:
      return nodeAddress(page, search, undefined);
  }
}

/** Where a node of the page being rendered opens, where closing it goes, and where a drawn line opens. */
export interface NodeAddresses {
  open(key: string): string;
  close(): string;
  line(from: string, to: string): string;
}

/** The page's node addresses (`nodeAddress`), from the pattern the router matched; outside a page, the current location. */
export function useNodeAddresses(): NodeAddresses {
  const page = usePage();
  const { pathname, search } = useLocation();
  return useMemo(() => {
    const stay = `${pathname}${withQuestion(search)}`;
    const at = (key: string | undefined) => (page === undefined ? stay : nodeAddress(page, search, key));
    return { open: (key) => at(key), close: () => at(undefined), line: (from, to) => (page === undefined ? stay : edgeAddress(page, search, from, to)) };
  }, [page, pathname, search]);
}

/**
 * View `to` from the journey page at `target` + `search`: the whole query, so the other views' own
 * settings stay for the way back. The node stays (or becomes `node` when given: Show in list, Show in graph, the Affects
 * link), except into the Walkthrough from another view: there a node's address means "opened from
 * a card", and a node opened elsewhere is no card of this pass. An edge stays only in the graph.
 */
export function screenSwitched(target: JourneyTarget, search: string, to: Screen, node?: string): string {
  const from = screenOf(target, new URLSearchParams(search));
  const params = new URLSearchParams(search);
  const keeps = node ?? (to === "walk" && from !== "walk" ? undefined : target.node);
  const edge = to === "graph" && node === undefined && keeps === undefined ? target.edge : undefined;
  return journeyPath({ journey: target.journey, node: keeps, edge, proposal: target.proposal }, to, params.toString());
}

/**
 * C14: the layer of `proposal` over the page at `target` + `search`, keeping the view on screen: the
 * Summary and the Walkthrough have no layer, so entering from them opens the layer's Graph; the
 * Graph, the List and the Timeline keep their query, so a proposal opened from the List is the List with
 * the layer on.
 */
export function layerEntry(target: JourneyTarget, search: string, proposal: string): string {
  const screen = layerScreen(screenOf({ journey: target.journey }, new URLSearchParams(search)));
  return journeyPath({ journey: target.journey, proposal }, screen, search);
}

/** C14: the page under the layer at `target` + `search` without it: the view on screen stays, written off the bare journey's own default. */
export function layerExit(target: JourneyTarget, search: string): string {
  return journeyPath({ journey: target.journey, node: target.node }, screenOf(target, new URLSearchParams(search)), search);
}

/** `search` with `?` in front when it holds anything, whichever way it was given. */
function withQuestion(search: string): string {
  return search === "" || search === "?" ? "" : search.startsWith("?") ? search : `?${search}`;
}
