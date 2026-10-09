// B13: the segments the stepper and the review name, read from the host: the published,
// unretired ones with their latest version's graph (the list step shows each one's size), and
// one version with its segment's header (what a local preview of the insertion reads).
import { useEffect, useMemo, useState } from "react";

import type { Route, RouteVersion } from "../data/host.ts";
import { isSegment, routeIndex } from "../data/reads.ts";
import { useLive, useSession } from "../data/react.ts";

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
  const published = useMemo(() => (index.status === "ready" ? index.value.filter((route) => isSegment(route) && route.header.retired !== true && route.latest_version != null) : []), [index]);
  const listed = useMemo(() => published.filter((route) => route.header.id !== except), [published, except]);
  const asked = listed.map((route) => `${route.header.id}@${String(route.latest_version)}`).join(" ");
  const [read, setRead] = useState<{ asked: string; offers: SegmentOffer[] } | undefined>(undefined);
  useEffect(() => {
    let live = true;
    Promise.all(
      listed.map(async (route): Promise<SegmentOffer> => {
        const latest = route.latest_version ?? 1;
        return { id: route.header.id, name: route.header.name, description: route.header.description ?? undefined, latest, version: await host.routeVersion(route.header.id, latest) };
      }),
    ).then(
      (offers) => {
        if (live) {
          setRead({ asked, offers });
        }
      },
      () => {
        if (live) {
          setRead({ asked, offers: [] });
        }
      },
    );
    return () => {
      live = false;
    };
    // `asked` names the segments and their latest versions.
  }, [host, asked]);
  const ready = index.status !== "loading" && read?.asked === asked;
  return { offers: ready ? read.offers : [], loading: !ready, leftOut: published.length > listed.length };
}

/** One version of a segment with its segment's header: what the engine reads to insert it. */
export interface SegmentVersion {
  segment: Route;
  version: RouteVersion;
}

/** The segment `route` at `version`, once read. */
export function useSegmentVersion(route: string | undefined, version: number | undefined): SegmentVersion | undefined {
  const { host } = useSession();
  const [read, setRead] = useState<{ key: string; found: SegmentVersion } | undefined>(undefined);
  const key = `${route ?? ""}@${String(version)}`;
  useEffect(() => {
    if (route === undefined || version === undefined) {
      return undefined;
    }
    let live = true;
    Promise.all([host.route(route), host.routeVersion(route, version)]).then(
      ([segment, found]) => {
        if (live) {
          setRead({ key, found: { segment, version: found } });
        }
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [host, route, version, key]);
  return read?.key === key ? read.found : undefined;
}

/** A segment's name by route id, from the route index; the id for one not listed. */
export function useSegmentNames(): (route: string) => string {
  const index = useLive("routes", routeIndex).view;
  return useMemo(() => {
    const names = new Map(index.status === "ready" ? index.value.map((route) => [route.header.id, route.header.name]) : []);
    return (route) => names.get(route) ?? route;
  }, [index]);
}

/** A segment's latest published version by route id, from the route index. */
export function useLatestVersions(): ReadonlyMap<string, number> {
  const index = useLive("routes", routeIndex).view;
  return useMemo(() => new Map(index.status === "ready" ? index.value.flatMap((route) => (route.latest_version == null ? [] : [[route.header.id, route.latest_version] as const])) : []), [index]);
}
