// What route detail shows (C17), worked out from what the host answers: each published
// version, newest first, with the journeys made from it. Route files leave and arrive as
// text (A13: YAML on disk).
import type { Schema } from "@cairn/client";

import type { JourneySummary, RouteRead } from "../data/reads.ts";
import type { Mutation } from "../data/writes.ts";
import { plural } from "../ui/words.ts";

/** One journey on a version, as route detail lists it. */
export interface VersionJourney {
  id: string;
  name: string;
  status: Schema<"JourneyStatus">;
}

export interface VersionRow {
  version: number;
  publishedAt: string;
  journeys: VersionJourney[];
}

/** C17: the route's versions, newest first, each with its journeys in name order. */
export function versionRows(read: RouteRead): VersionRow[] {
  const byId = new Map<string, JourneySummary>(read.journeys.map((summary) => [summary.id, summary]));
  const rows = read.detail.versions.map((version) => ({
    version: version.version,
    publishedAt: version.published_at,
    journeys: version.journeys
      .map((id): VersionJourney => {
        const summary = byId.get(id);
        return { id, name: summary?.name ?? id, status: summary?.status ?? "active" };
      })
      .sort((left, right) => left.name.localeCompare(right.name) || left.id.localeCompare(right.id)),
  }));
  return rows.reverse();
}

/** A11: what an author can do to the route as it is: each one route patch. */
export interface RouteMoves {
  /** Open a draft extending the latest version (or an empty route's first). */
  open: boolean;
  publish: boolean;
  discard: boolean;
  /** Retire, or bring back (A19). */
  retire: boolean;
}

export function routeMoves(route: Schema<"Route">): RouteMoves {
  const draft = route.draft != null;
  return { open: !draft, publish: draft, discard: draft, retire: route.header.retired !== true };
}

export function openDraft(): Mutation {
  return { op: "open_draft", source: "edit" };
}

export function retireMutation(retired: boolean): Mutation {
  return { op: "set_route_retired", retired };
}

/** A13: the name an exported file is saved under. */
export function exportName(route: string, version: number | undefined): string {
  return `${route}-${version === undefined ? "draft" : `v${String(version)}`}.yaml`;
}

/** Hands the browser `text` to save as `name`. */
export function download(name: string, text: string): void {
  const url = URL.createObjectURL(new Blob([text], { type: "application/yaml" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.click();
  setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 0);
}

/** The routes index's types: every one, only routes (the processes a journey is started from), or only segments (the pieces inserted into them). */
export type RouteType = "all" | "routes" | "segments";

/** The type `?type=` names; every type when it names none. */
export function typeOf(param: string | null): RouteType {
  return param === "routes" || param === "segments" ? param : "all";
}

/** The kind of route the host is asked to list for `type`: none for every type. */
export function kindOf(type: RouteType): Schema<"RouteKind"> | undefined {
  return type === "all" ? undefined : type === "segments" ? "segment" : "process";
}

/** How many active journeys each route has: counted from the journeys' own lineage, never derived. */
export function activeJourneyCounts(journeys: readonly JourneySummary[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const journey of journeys) {
    if (journey.status === "active" && journey.lineage != null) {
      counts.set(journey.lineage.route, (counts.get(journey.lineage.route) ?? 0) + 1);
    }
  }
  return counts;
}

/** What a route or segment is used by: a route's active journeys, a segment's insertions (B13). */
export type InUse = { kind: "journeys"; count: number } | { kind: "places"; places: number };

/** The use in words: `2 active journeys`, `Used in 6 places`, and what none says. */
export function inUseWords(use: InUse): string {
  if (use.kind === "journeys") {
    return use.count === 0 ? "No active journeys" : plural(use.count, "active journey");
  }
  if (use.places === 0) {
    return "Not used yet";
  }
  return `Used in ${plural(use.places, "place")}`;
}

/** A19, A21: whether journeys start from the route's published versions: not from a segment, and not once the route is retired. */
export function startsJourneys(header: Pick<Schema<"RouteSummary">["header"], "kind" | "retired">): boolean {
  return header.kind !== "segment" && header.retired !== true;
}

/** The version a journey starts from by default: the latest published, of a route that starts journeys; none otherwise. A draft being open does not stop it. */
export function startableVersion(route: Schema<"RouteSummary">): number | undefined {
  return startsJourneys(route.header) ? (route.latest_version ?? undefined) : undefined;
}
