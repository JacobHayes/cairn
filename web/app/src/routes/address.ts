// The addresses of the route screens (C17): the route index and a route's detail. A route's
// canvas keeps its own address (screens/RouteCanvasPage.tsx).

/** The route index. */
export const ROUTES_PATH = "/routes";

/** C17: route `route`'s versions with the journeys on each. */
export function routeDetailPath(route: string): string {
  return `/routes/${route}/versions`;
}
