// The addresses of the route screens (C17): the routes index, `/routes`, which lists routes and
// segments (`?type=routes|segments` keeps it to one), the new-route form, `/routes/new`, and a
// route's detail. A route's canvas keeps its own address (screens/RouteCanvasPage.tsx).

/** What the rail links to: the routes index, every type. */
export const ROUTES_INDEX_PATH = "/routes";

/** The routes index kept to routes. */
export const ROUTES_PATH = "/routes?type=routes";

/** The routes index kept to segments. */
export const SEGMENTS_PATH = "/routes?type=segments";

/** A11, A12: the new-route form (`?kind=segment` for a segment). A route's id `new` is shadowed by it, so the form refuses it. */
export function newRoutePath(kind?: "segment"): string {
  return kind === "segment" ? "/routes/new?kind=segment" : "/routes/new";
}

/** C17: route `route`'s versions with the journeys on each. */
export function routeDetailPath(route: string): string {
  return `/routes/${route}/versions`;
}
