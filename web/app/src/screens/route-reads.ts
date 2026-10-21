// A route page's reads (ARCHITECTURE, Web UI: reads), as keyed reads (data/keyed-read.ts): the
// route's draft or version, kept current while shown (H6), and that graph derived once in the
// derive worker as a journey in which nothing has happened and held there, like a journey's
// document, under a slot of its own for each load. The views read their level, trace and
// schedule from that derivation (canvas/hooks.ts). A refetch that fails keeps the good graph on screen and counts the canvas as behind on the
// sync chip; a route that no longer exists is gone (A19).
import type { Schema } from "@cairn/client";

import type { DerivationKey } from "@cairn/wasm";

import type { Deployment, Deriver, Host, Route } from "../data/host.ts";
import { beforeWork, DERIVE, HOST_READ, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { isRoute, routeOf } from "../data/reads.ts";
import type { GraphNode } from "../detail/model.ts";

/** The graph a route's page shows: its draft, or one published version. */
export interface Shown {
  route: Route;
  graph: Schema<"Graph">;
  /** "draft", or the version's number. */
  of: "draft" | number;
}

const NO_NODES: GraphNode[] = [];

/** The nodes of the shown graph; one list for a graph with none, so what is derived from them is not made afresh with every render. */
export function nodesOf(shown: { graph: Schema<"Graph"> }): GraphNode[] {
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
  return { of: [id, version], freshness, args: { host, id, version } };
}

/** What a derived graph is read from: the thing it belongs to carries its graph, and `reads` names what its projections are of. */
interface DerivedArgs<S extends { graph: Schema<"Graph"> }> {
  deriver: Deriver;
  subject: S;
  deployment: Deployment;
  /** What the projections of this graph are of: the same across the loads of one subject, so a draft edit keeps the last answers on screen until the new ones arrive. */
  reads: string;
}

/**
 * A graph derived once in the worker and held there under a slot of its own for each load (a
 * slot is never reused, so what is read through one load's key is never answered from the load
 * that replaced it), together with the subject it was derived for: the views draw the subject's
 * nodes beside the derivation's level, trace and schedule, so a newer graph is never paired with
 * an older derivation. The handle is released when the read drops the value.
 */
export type Derived<S extends { graph: Schema<"Graph"> }> = S & { key: DerivationKey; reads: string; free: () => void };

let loads = 0;

/** A source of derived graphs of `S`: the route page's shown graph, the insertion preview's graph. */
export function derivedSource<S extends { graph: Schema<"Graph"> }>(): ReadSource<DerivedArgs<S>, Derived<S>> {
  return {
    policy: DERIVE,
    read: async ({ deriver, subject, deployment, reads }, at) => {
      loads += 1;
      const loaded = await deriver.loadRoute({ graph: subject.graph, deployment }, `${reads}#${String(loads)}`);
      let freed = false;
      const free = () => {
        if (!freed) {
          freed = true;
          void deriver.release(loaded.handle).catch(() => undefined);
        }
      };
      // The read was dropped while the graph was deriving (or is, before the value is adopted): nothing will own the derivation.
      at.signal.addEventListener("abort", free, { once: true });
      try {
        beforeWork(at);
        return { ...subject, key: loaded.key, reads, free };
      } catch (thrown) {
        free();
        throw thrown;
      }
    },
    release(value) {
      value.free();
    },
  };
}

/** The route page's shown graph, derived and held. */
export const ROUTE_DERIVED = derivedSource<Shown>();

/**
 * The key of a shown graph's derivation: the route and what it shows are what it is of; the
 * graph and the deployment are what it is derived from, so an edit to a draft derives it again
 * and keeps the last derivation on screen meanwhile.
 */
export function routeDerivedKey(deriver: Deriver, shown: Shown | undefined, deployment: Deployment | undefined): ReadKey<DerivedArgs<Shown>> | undefined {
  if (shown === undefined || deployment === undefined) {
    return undefined;
  }
  const reads = `route:${shown.route.header.id}:${String(shown.of)}`;
  return { of: [reads], freshness: [shown.graph, deployment.revision], args: { deriver, subject: shown, deployment, reads } };
}
