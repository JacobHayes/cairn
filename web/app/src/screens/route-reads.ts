// A route page's reads (ARCHITECTURE, Web UI: reads), as keyed reads (data/keyed-read.ts): the
// route's draft or version, kept current while shown (H6), and the level and the schedule of that
// graph from the derive worker. A refetch that fails keeps the good graph on screen and counts the canvas as behind on the
// sync chip; a route that no longer exists is gone (A19).
import type { Schema } from "@cairn/client";

import type { RouteSchedule } from "@cairn/wasm";

import type { Deployment, Deriver, Host, Level, Route } from "../data/host.ts";
import { HOST_READ, PROJECTION, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { isRoute, routeOf } from "../data/reads.ts";
import type { GraphNode } from "../detail/model.ts";
import { levelQueryOf, type LevelRequest } from "../journeys/requests.ts";

/** The graph a route's page shows: its draft, or one published version. */
export interface Shown {
  route: Route;
  graph: Schema<"Graph">;
  /** "draft", or the version's number. */
  of: "draft" | number;
}

const NO_NODES: GraphNode[] = [];

/** The nodes of the shown graph; one list for a graph with none, so what is derived from them is not made afresh with every render. */
export function nodesOf(shown: Shown): GraphNode[] {
  return shown.graph.nodes ?? NO_NODES;
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

/** The key of a route's graph: the same read for the same route and version; `freshness` is whatever else says it is to be read again (a proposal that created the route applying). */
export function routeGraphKey(host: Host, id: string, version: number | undefined, freshness: readonly unknown[] = []): ReadKey<{ host: Host; id: string; version: number | undefined }> {
  return { identity: [id, version], freshness, args: { host, id, version } };
}

/** What a route's level is derived from. */
interface LevelArgs {
  deriver: Deriver;
  shown: Shown;
  deployment: Deployment;
  today: string;
  request: LevelRequest;
}

export const ROUTE_LEVEL: ReadSource<LevelArgs, Level> = {
  policy: PROJECTION,
  read: ({ deriver, shown, deployment, today, request }) => deriver.routeLevel({ graph: shown.graph, deployment, today, query: levelQueryOf(request) }),
};

/**
 * The key of a shown graph's level for `request` (journeys/requests.ts, `levelRequestOf`, the one
 * place a view state becomes a level query, a journey's or a route's): the route, what it shows
 * and the request are the identity (another request's level laid out under this one would hint
 * its first layout with another graph's positions); the graph, the deployment and the day are
 * what it is read from.
 */
export function routeLevelKey(deriver: Deriver, shown: Shown, request: LevelRequest | undefined, deployment: Deployment | undefined, today: string): ReadKey<LevelArgs> | undefined {
  if (deployment === undefined || request === undefined) {
    return undefined;
  }
  return {
    identity: [shown.route.header.id, shown.of, JSON.stringify(request)],
    freshness: [shown.graph, deployment.revision, today],
    args: { deriver, shown, deployment, today, request },
  };
}

/** What a route's schedule is derived from. */
interface ScheduleArgs {
  deriver: Deriver;
  shown: Shown;
  deployment: Deployment;
}

/** The earliest days of a shown graph's nodes from its start (C13). */
export const ROUTE_SCHEDULE: ReadSource<ScheduleArgs, RouteSchedule> = {
  policy: PROJECTION,
  read: ({ deriver, shown, deployment }) => deriver.routeSchedule({ graph: shown.graph, deployment }),
};

/** The key of a shown graph's schedule: the route and what it shows are the identity; the graph and the deployment are what it is read from. */
export function routeScheduleKey(deriver: Deriver, shown: Shown, deployment: Deployment | undefined): ReadKey<ScheduleArgs> | undefined {
  if (deployment === undefined) {
    return undefined;
  }
  return { identity: [shown.route.header.id, shown.of], freshness: [shown.graph, deployment.revision], args: { deriver, shown, deployment } };
}
