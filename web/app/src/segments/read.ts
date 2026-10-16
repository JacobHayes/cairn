// B13: the segments the stepper and the review name, read from the host: the published,
// unretired ones with their latest version's graph (the list step shows each one's size), and
// one version with its segment's header (what a local preview of the insertion reads).
import { useMemo } from "react";

import type { Host, Route, RouteVersion } from "../data/host.ts";
import { HOST_READ, valueOf, type ReadSource } from "../data/keyed-read.ts";
import { isSegment, routeIndex, type RouteSummary } from "../data/reads.ts";
import { useKeyedRead, useLive, useSession } from "../data/react.ts";

/** A segment the stepper offers: published, not retired, with its latest version read. */
export interface SegmentOffer {
  id: string;
  name: string;
  description: string | undefined;
  latest: number;
  version: RouteVersion;
}

/** The segments an insertion may use (A19: a retired one is not offered), each with its latest version; `except` is the route being edited, and `leftOut` says it was published and so would have been offered. */
export function useSegmentOffers(except: string | undefined): { offers: SegmentOffer[]; loading: boolean; leftOut: boolean } {
  const { host } = useSession();
  const index = useLive("routes", routeIndex).view;
  const indexed = valueOf(index);
  const published = useMemo(() => (indexed === undefined ? [] : indexed.filter((route) => isSegment(route) && route.header.retired !== true && route.latest_version != null)), [indexed]);
  const listed = useMemo(() => published.filter((route) => route.header.id !== except), [published, except]);
  // The segments and their latest versions are what the offers are read from.
  const asked = listed.map((route) => `${route.header.id}@${String(route.latest_version)}`).join(" ");
  const settled = index.status !== "idle" && index.status !== "loading";
  const offered = useKeyedRead(OFFERS, settled ? { identity: ["segment-offers"], freshness: [asked], args: { host, listed } } : undefined).view;
  const offers = offered.status === "current" ? offered.value : [];
  const loading = offered.status !== "current" && offered.status !== "failed";
  return { offers, loading, leftOut: published.length > listed.length };
}

/** What the offers are read from: the segments listed, each read at its latest version. */
const OFFERS: ReadSource<{ host: Host; listed: RouteSummary[] }, SegmentOffer[]> = {
  policy: HOST_READ,
  read: ({ host, listed }) =>
    Promise.all(
      listed.map(async (route): Promise<SegmentOffer> => {
        const latest = route.latest_version ?? 1;
        return { id: route.header.id, name: route.header.name, description: route.header.description ?? undefined, latest, version: await host.routeVersion(route.header.id, latest) };
      }),
    ),
};

/** One version of a segment with its segment's header: what the engine reads to insert it. */
export interface SegmentVersion {
  segment: Route;
  version: RouteVersion;
}

/** A segment version is immutable: read once per route and version. */
const SEGMENT_VERSION: ReadSource<{ host: Host; route: string; version: number }, SegmentVersion> = {
  policy: HOST_READ,
  read: async ({ host, route, version }) => {
    const [segment, found] = await Promise.all([host.route(route), host.routeVersion(route, version)]);
    return { segment, version: found };
  },
};

/** The segment `route` at `version`, once read. */
export function useSegmentVersion(route: string | undefined, version: number | undefined): SegmentVersion | undefined {
  const { host } = useSession();
  const key = route === undefined || version === undefined ? undefined : { identity: [route, version], freshness: [], args: { host, route, version } };
  return valueOf(useKeyedRead(SEGMENT_VERSION, key).view);
}

/** A segment's name by route id, from the route index; the id for one not listed. */
export function useSegmentNames(): (route: string) => string {
  const indexed = valueOf(useLive("routes", routeIndex).view);
  return useMemo(() => {
    const names = new Map((indexed ?? []).map((route) => [route.header.id, route.header.name] as const));
    return (route) => names.get(route) ?? route;
  }, [indexed]);
}

/** A segment's latest published version by route id, from the route index. */
export function useLatestVersions(): ReadonlyMap<string, number> {
  const indexed = valueOf(useLive("routes", routeIndex).view);
  return useMemo(() => new Map((indexed ?? []).flatMap((route) => (route.latest_version == null ? [] : [[route.header.id, route.latest_version] as const]))), [indexed]);
}
