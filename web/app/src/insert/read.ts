// B13: the routes the stepper and the review name, read from the host: the published,
// unretired ones from the route index, and one version with its route's header (what a local preview of the insertion reads).
import { useMemo } from "react";

import type { Schema } from "@cairn/client";

import type { Graph } from "../authoring/graph.ts";
import type { Deriver, Host, Route, RouteVersion } from "../data/host.ts";
import { DERIVE, HOST_READ, valueOf, type ReadSource } from "../data/keyed-read.ts";
import { routeIndex } from "../data/reads.ts";
import { useKeyedRead, useLive, useSession } from "../data/react.ts";

/** A route the stepper offers: published and not retired, at its latest version. */
export interface RouteOffer {
  id: string;
  name: string;
  description: string | undefined;
  latest: number;
}

/** The routes an insertion may use (A19: a retired one is not offered). */
export function useRouteOffers(): { offers: RouteOffer[]; loading: boolean } {
  const index = useLive("routes", routeIndex).view;
  const indexed = valueOf(index);
  const offers = useMemo(
    () =>
      (indexed ?? [])
        .filter((route) => route.header.retired !== true && route.latest_version != null)
        .map((route): RouteOffer => ({ id: route.header.id, name: route.header.name, description: route.header.description ?? undefined, latest: route.latest_version ?? 1 })),
    [indexed],
  );
  const loading = index.status === "idle" || index.status === "loading";
  return { offers, loading };
}

/** One version of a route with its header: what the engine reads to insert it. */
export interface RouteCopy {
  route: Route;
  version: RouteVersion;
}

/** A route version is immutable: read once per route and version. */
const ROUTE_COPY: ReadSource<{ host: Host; route: string; version: number }, RouteCopy> = {
  policy: HOST_READ,
  read: async ({ host, route, version }) => {
    const [whole, found] = await Promise.all([host.route(route), host.routeVersion(route, version)]);
    return { route: whole, version: found };
  },
};

/** The route `route` at `version`, once read. */
export function useRouteCopy(route: string | undefined, version: number | undefined): RouteCopy | undefined {
  const { host } = useSession();
  const key = route === undefined || version === undefined ? undefined : { of: [route, version], freshness: [], args: { host, route, version } };
  return valueOf(useKeyedRead(ROUTE_COPY, key).view);
}

interface PlanArgs {
  deriver: Deriver;
  host: Graph;
  route: Graph;
  ask: Schema<"InsertionAsk">;
}

const INSERTION_PLAN: ReadSource<PlanArgs, Schema<"InsertionPlan">> = {
  policy: DERIVE,
  read: ({ deriver, host, route, ask }) => deriver.insertionPlan(host, route, ask),
};

/**
 * B13, C19: the engine's plan for inserting `route` into `host` given the people choices: where each role and kind lands,
 * which nodes are left out and which roots are placed. The last answer stays for display while a new one is worked out, and `current` says when it is no longer the last.
 */
export function useInsertionPlan(host: Graph, route: Graph | undefined, ask: Schema<"InsertionAsk">): { plan: Schema<"InsertionPlan">; current: boolean } | undefined {
  const { deriver } = useSession();
  const key = route === undefined ? undefined : { of: ["insertion"], freshness: [JSON.stringify(host), JSON.stringify(route), JSON.stringify(ask)], args: { deriver, host, route, ask } };
  const { view } = useKeyedRead(INSERTION_PLAN, key);
  const plan = valueOf(view);
  return plan === undefined ? undefined : { plan, current: view.status === "current" };
}

/** A route's name by route id, from the route index; the id for one not listed. */
export function useRouteNames(): (route: string) => string {
  const indexed = valueOf(useLive("routes", routeIndex).view);
  return useMemo(() => {
    const names = new Map((indexed ?? []).map((route) => [route.header.id, route.header.name] as const));
    return (route) => names.get(route) ?? route;
  }, [indexed]);
}

/** A route as the index knows it: its name, `"gone"` once the index is read and lacks it (it was deleted), none while the index is being read. */
export function useRouteLookup(): (route: string) => { name: string } | "gone" | undefined {
  const indexed = valueOf(useLive("routes", routeIndex).view);
  return useMemo(() => {
    const names = new Map((indexed ?? []).map((route) => [route.header.id, route.header.name] as const));
    return (route) => (indexed === undefined ? undefined : names.has(route) ? { name: names.get(route) ?? route } : "gone");
  }, [indexed]);
}
