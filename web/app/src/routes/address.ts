// The addresses of the route screens (C17): the routes index, `/routes`, which lists routes and
// segments (`?type=routes|segments` keeps it to one), the new-route form, `/routes/new`, and a
// route's page. A route is one page (screens/RoutePage.tsx) whose address says which version its
// views show and which view is open:
//
//   /routes/<id>                      the route as it stands: its draft when one is open, else its latest version
//   /routes/<id>?version=<n>          published version n
//   /routes/<id>/nodes/<key>          a node of the draft (its form) or of the route as it stands
//   /routes/<id>/nodes/<key>?version=<n>   a node of published version n (read only)
//   /routes/<id>/edges/<from>~<to>[?version=<n>]   the line the graph shown draws between those two cards (always the Graph)
//   /routes/<id>?view=list|timeline   the view (the Graph when none; written only off the default)
//   /routes/<id>/proposals/<pid>[/nodes/<key>]   the route with a proposal's layer on (C14); it reads no `version`
//
// `version` absent names "the route as it currently stands" and follows it as drafts open and
// versions publish; a version a link names is always written, the latest included, because a
// version never changes and its link must keep meaning it. Beside them the view state every view
// shares (journeys/view.ts) and the graph's own settings ride in the query. The patterns are address.ts's.
import { pathTo } from "../address.ts";
import { PROJECTIONS, type Screen } from "../journeys/address.ts";

/** What the rail links to: the routes index, every type. */
export const ROUTES_INDEX_PATH = pathTo("routes");

/** The routes index kept to routes. */
export const ROUTES_PATH = `${ROUTES_INDEX_PATH}?type=routes`;

/** A11, A12: the new-route form (`?kind=segment` for a segment). A route's id `new` is shadowed by it, so the form refuses it. */
export function newRoutePath(kind?: "segment"): string {
  return kind === "segment" ? `${pathTo("routeNew")}?kind=segment` : pathTo("routeNew");
}

/** Where on a route's page an address points: a published version (none is the route as it stands), a draft node, the view, and a proposal whose layer is on (which reviews the route as it stands, so names no version). */
export interface RouteSpot {
  version?: number | undefined;
  node?: string | undefined;
  /** A drawn line, as `<from>~<to>`; a node wins over it. */
  edge?: string | undefined;
  screen?: Screen | undefined;
  proposal?: string | undefined;
}

/** The version `version=` names: a whole number from 1, else none. */
export function versionIn(params: URLSearchParams): number | undefined {
  const text = params.get("version");
  const number = Number(text);
  return text !== null && Number.isInteger(number) && number >= 1 ? number : undefined;
}

/** The view `view=` names, among a route's; the Graph when it names none of them. */
export function routeScreenOf(params: URLSearchParams): Screen {
  const view = params.get("view");
  return PROJECTIONS.find((each) => each === view) ?? "graph";
}

/** The version and the view an address names, to carry to another place on the same route. */
export function spotOf(params: URLSearchParams): { version: number | undefined; screen: Screen } {
  return { version: versionIn(params), screen: routeScreenOf(params) };
}

/**
 * Route `route`'s page at `spot`, keeping `search`'s other parameters (the view state, the
 * graph's settings) in their order. `version` and `view` are written from `spot`, never kept
 * from `search`: a version when one is named, the view only off the Graph. A node is written
 * whatever the version: the address names the node of the graph the page shows. A line is written
 * the same way, when no node is.
 */
export function routePath(route: string, spot: RouteSpot = {}, search = ""): string {
  const kept = new URLSearchParams(search);
  kept.delete("version");
  kept.delete("view");
  const params = new URLSearchParams();
  if (spot.version !== undefined) {
    params.set("version", String(spot.version));
  }
  if (spot.screen !== undefined && spot.screen !== "graph") {
    params.set("view", spot.screen);
  }
  for (const [name, value] of kept) {
    params.append(name, value);
  }
  const query = params.toString();
  if (spot.proposal !== undefined) {
    params.delete("version");
    const proposal = spot.proposal;
    const path = spot.node === undefined ? pathTo("routeProposal", { route, proposal }) : pathTo("routeProposalNode", { route, proposal, key: spot.node });
    const text = params.toString();
    return `${path}${text === "" ? "" : `?${text}`}`;
  }
  const path = spot.node !== undefined ? pathTo("routeNode", { route, key: spot.node }) : spot.edge !== undefined ? pathTo("routeEdge", { route, edge: spot.edge }) : pathTo("route", { route });
  return `${path}${query === "" ? "" : `?${query}`}`;
}

/**
 * View `to` from the route page at `search`: the whole query, so every view's own settings stay
 * for the way back, and the version and the node stay.
 */
export function routeSwitched(route: string, search: string, to: Screen, node: string | undefined, proposal?: string): string {
  const params = new URLSearchParams(search);
  const { version } = spotOf(params);
  return routePath(route, { version, node, screen: to, proposal }, params.toString());
}

/** C14: the route's page without the layer at `search`: the view stays; the version is the route as it stands. */
export function routeLayerExit(route: string, search: string): string {
  const params = new URLSearchParams(search);
  return routePath(route, { screen: routeScreenOf(params) }, search);
}
