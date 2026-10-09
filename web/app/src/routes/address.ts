// The addresses of the route screens (C17): the Library, whose index lists routes, and a
// route's detail. A route's canvas keeps its own address (screens/RouteCanvasPage.tsx).

/** The Library's routes (`/routes` redirects here). */
export const ROUTES_PATH = "/library?type=routes";

/** C17: route `route`'s versions with the journeys on each. */
export function routeDetailPath(route: string): string {
  return `/routes/${route}/versions`;
}
