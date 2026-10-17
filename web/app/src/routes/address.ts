// The addresses of the route screens (C17): the routes index, `/routes`, which lists routes and
// segments (`?type=routes|segments` keeps it to one), the new-route form, `/routes/new`, and a
// route's detail. A route's canvas keeps its own address (screens/RouteCanvasPage.tsx). The
// patterns are address.ts's.
import { pathTo } from "../address.ts";
import { paramsOf, searchOf, type CanvasView } from "../canvas/settings.ts";

/** What the rail links to: the routes index, every type. */
export const ROUTES_INDEX_PATH = pathTo("routes");

/** The routes index kept to routes. */
export const ROUTES_PATH = `${ROUTES_INDEX_PATH}?type=routes`;

/** The routes index kept to segments. */
export const SEGMENTS_PATH = `${ROUTES_INDEX_PATH}?type=segments`;

/** A11, A12: the new-route form (`?kind=segment` for a segment). A route's id `new` is shadowed by it, so the form refuses it. */
export function newRoutePath(kind?: "segment"): string {
  return kind === "segment" ? `${pathTo("routeNew")}?kind=segment` : pathTo("routeNew");
}

/** C17: route `route`'s versions with the journeys on each. */
export function routeDetailPath(route: string): string {
  return pathTo("routeVersions", { route });
}

/** The path of a route's canvas: its draft, or `version`, keeping what the canvas shows. */
export function routeCanvasPath(route: string, version: number | undefined, view: CanvasView): string {
  const params = paramsOf(view);
  if (version !== undefined) {
    params.set("version", String(version));
  }
  const query = params.toString();
  return `${version === undefined ? pathTo("routeDraft", { route }) : pathTo("route", { route })}${query === "" ? "" : `?${query}`}`;
}

/** A draft node's form in the inspector beside the route's canvas, keeping what the canvas shows. */
export function routeNodePath(route: string, view: CanvasView, node: string): string {
  return `${pathTo("routeDraftNode", { route, key: node })}${searchOf(view)}`;
}
