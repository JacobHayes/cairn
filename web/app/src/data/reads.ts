// The live reads the screens around a journey share (H6): a filtered journey index, the
// route index, and a route's detail, each watching what it shows and refetching on a tick
// newer than it holds. Every page is read, so a screen shows the whole answer.
import type { RevisionOf, Schema } from "@cairn/client";

import type { JourneyIndexQuery } from "./host.ts";
import { journeyOf } from "./journeys.ts";
import { allPages, type LiveSpec } from "./live.ts";
import type { Session } from "./session.ts";

export type JourneySummary = Schema<"JourneySummary">;
export type RouteSummary = Schema<"RouteSummary">;
export type RouteDetail = Schema<"RouteDetail">;

const isJourney = (of: RevisionOf): boolean => "domain" in of && typeof of.domain === "object" && "journey" in of.domain;
export const isRoute = (of: RevisionOf, id?: string): boolean =>
  "domain" in of && typeof of.domain === "object" && "route" in of.domain && (id === undefined || of.domain.route === id);
export const routeOf = (id: string): RevisionOf => ({ domain: { route: id } });

/** The key a journey index read is made afresh for. */
export function indexKey(query: JourneyIndexQuery | undefined): string {
  return `journeys:${JSON.stringify(query ?? null)}`;
}

/**
 * C16: every journey `query` asks for, kept current: a tick for any journey newer than held,
 * or one not held (a journey just created, or newly matching), refetches it; so does the
 * deployment moving (a merge changes which journeys refer to the caller). No query is nothing
 * to ask for: an empty answer.
 */
export function journeyIndex(query: JourneyIndexQuery | undefined) {
  return (session: Session): LiveSpec<JourneySummary[]> => ({
    watching: ["journeys", "deployment"],
    about: (of) => isJourney(of) || ("domain" in of && of.domain === "deployment"),
    fetch: () =>
      query === undefined ? Promise.resolve([]) : allPages<JourneySummary, string>((after) => session.host.journeys(after === undefined ? query : { ...query, after })),
    holds: (items) => items.map((item) => ({ of: journeyOf(item.id), revision: item.revision })),
  });
}

/** The references of each identity asked about, by identity key. */
export type ReferencesByIdentity = Readonly<Record<string, Schema<"IdentityReferences">>>;

/** The key a references read is made afresh for. */
export function referencesKey(identities: readonly string[]): string {
  return `identity-references:${identities.join(",")}`;
}

/**
 * E6: what refers to each of `identities` (the journeys, route drafts and route versions naming
 * it, directly or through a merged key), kept current: a journey ticking past what is held, a
 * route moving, or the deployment moving (a merge or delete) reads it again.
 */
export function referencesOf(identities: readonly string[]) {
  return (session: Session): LiveSpec<ReferencesByIdentity> => ({
    watching: ["journeys", "routes", "deployment"],
    about: (of) => isJourney(of) || isRoute(of) || ("domain" in of && of.domain === "deployment"),
    fetch: async () => Object.fromEntries(await Promise.all(identities.map(async (identity) => [identity, await session.host.identityReferences(identity)] as const))),
    holds: (by) => Object.values(by).flatMap((each) => each.journeys.map((journey) => ({ of: journeyOf(journey.id), revision: journey.revision }))),
  });
}

/** The route index, kept current. */
export function routeIndex(session: Session): LiveSpec<RouteSummary[]> {
  return {
    watching: ["routes"],
    about: (of) => isRoute(of),
    fetch: () => allPages<RouteSummary, string>((after) => session.host.routes(after)),
    holds: (items) => items.map((item) => ({ of: routeOf(item.header.id), revision: item.revision })),
  };
}

/** What route detail shows: the route with its draft, its versions' journeys, and those journeys' summaries. */
export interface RouteRead {
  route: Schema<"Route">;
  detail: RouteDetail;
  journeys: JourneySummary[];
}

/**
 * C17: route `id` with its versions and the journeys on each, kept current: its own ticks
 * (publishing, a draft, retiring) and any journey's (one started or deleted).
 */
export function routeRead(id: string) {
  return (session: Session): LiveSpec<RouteRead> => ({
    watching: [`route:${id}`, "journeys"],
    about: (of) => isRoute(of, id) || isJourney(of),
    fetch: async () => {
      const [route, detail, journeys] = await Promise.all([
        session.host.route(id),
        session.host.routeDetail(id),
        allPages<JourneySummary, string>((after) => session.host.journeys(after === undefined ? { route: id } : { route: id, after })),
      ]);
      return { route, detail, journeys };
    },
    holds: (read) => [
      { of: routeOf(id), revision: Math.min(read.route.revision, read.detail.revision) },
      ...read.journeys.map((item) => ({ of: journeyOf(item.id), revision: item.revision })),
    ],
  });
}
