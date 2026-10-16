// A route canvas's two reads (ARCHITECTURE, Web UI: reads), as keyed reads (data/keyed-read.ts): the
// route's draft or version, kept current while shown (H6), and the level of that graph from the derive
// worker. A refetch that fails keeps the good graph on screen and counts the canvas as behind on the
// sync chip; a route that no longer exists is gone (A19).
import type { Schema } from "@cairn/client";

import { routeKindsAt } from "../canvas/ladder.ts";
import type { Step } from "../canvas/model.ts";
import { layoutViewOf, type CanvasView } from "../canvas/settings.ts";
import type { Deployment, Deriver, Host, Level, Route } from "../data/host.ts";
import { HOST_READ, PROJECTION, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { isRoute, routeOf } from "../data/reads.ts";

/** The graph a route's canvas shows: its draft, or one published version. */
export interface Shown {
  route: Route;
  graph: Schema<"Graph">;
  /** "draft", or the version's number. */
  of: "draft" | number;
}

/** What a route's read found: a graph to draw, or a route whose only draft was discarded and so has nothing to draw. */
export type RouteFound = { empty: Route } | { shown: Shown };

const revisionOf = (found: RouteFound): number => ("empty" in found ? found.empty.revision : found.shown.route.revision);

/** Route `id`'s draft, or `version` (the latest when no draft is open). */
export const ROUTE_GRAPH: ReadSource<{ host: Host; id: string; version: number | undefined }, RouteFound> = {
  policy: HOST_READ,
  read: async ({ host, id, version }) => {
    const route = await host.route(id);
    const latest = Math.max(0, ...(route.versions ?? []));
    if (route.draft != null && version === undefined) {
      return { shown: { route, graph: route.draft.graph, of: "draft" } };
    }
    if (version === undefined && latest === 0) {
      return { empty: route };
    }
    const number = version ?? latest;
    return { shown: { route, graph: (await host.routeVersion(id, number)).graph, of: number } };
  },
  ticks: ({ id }) => ({
    watching: [`route:${id}`],
    about: (of) => isRoute(of, id),
    holds: (found) => [{ of: routeOf(id), revision: revisionOf(found) }],
    subject: routeOf(id),
    reportsLag: true,
  }),
};

/** The key of a route's graph: the same read for the same route and version. */
export function routeGraphKey(host: Host, id: string, version: number | undefined): ReadKey<{ host: Host; id: string; version: number | undefined }> {
  return { identity: [id, version], freshness: [], args: { host, id, version } };
}

/** What a route's level is derived from. */
interface LevelArgs {
  deriver: Deriver;
  shown: Shown;
  deployment: Deployment;
  today: string;
  step: Step;
  view: CanvasView;
}

/** The level of a shown graph, and the graph and layout view it answers. */
export interface RouteLevel {
  level: Level;
  shown: Shown;
}

export const ROUTE_LEVEL: ReadSource<LevelArgs, RouteLevel> = {
  policy: PROJECTION,
  read: async ({ deriver, shown, deployment, today, step, view }) => ({
    level: await deriver.routeLevel({ graph: shown.graph, deployment, today, shown: routeKindsAt(step), ...(view.container === undefined ? {} : { container: view.container }) }),
    shown,
  }),
};

/**
 * The key of a shown graph's level: the route, what it shows and the view it is laid out for are
 * the identity (another view's level laid out under this one would hint its first layout with
 * another graph's positions); the graph, the deployment and the day are what it is read from.
 */
export function routeLevelKey(deriver: Deriver, shown: Shown, view: CanvasView, step: Step, deployment: Deployment | undefined, today: string): ReadKey<LevelArgs> | undefined {
  if (deployment === undefined) {
    return undefined;
  }
  return {
    identity: [shown.route.header.id, shown.of, layoutViewOf(view, step)],
    freshness: [shown.graph, deployment.revision, today],
    args: { deriver, shown, deployment, today, step, view },
  };
}
