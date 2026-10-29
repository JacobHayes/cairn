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

/** The use in words: `2 active journeys`, and what none says. */
export function inUseWords(count: number): string {
  return count === 0 ? "No active journeys" : plural(count, "active journey");
}

/** A19: whether journeys start from the route's published versions: not once the route is retired. */
export function startsJourneys(header: Pick<Schema<"RouteSummary">["header"], "retired">): boolean {
  return header.retired !== true;
}

/** The version a journey starts from by default: the latest published, of a route that starts journeys; none otherwise. A draft being open does not stop it. */
export function startableVersion(route: Schema<"RouteSummary">): number | undefined {
  return startsJourneys(route.header) ? (route.latest_version ?? undefined) : undefined;
}
