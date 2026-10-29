// C17: route detail lists each version, newest first, with the journeys made from it.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import type { RouteRead } from "../data/reads.ts";
import { activeJourneyCounts, exportName, inUseWords, routeMoves, startableVersion, versionRows } from "./model.ts";

type Summary = Schema<"JourneySummary">;

function summary(id: string, version: number): Summary {
  return {
    id,
    name: id,
    status: "active",
    revision: 1,
    created_at: "2026-10-01T00:00:00Z",
    lineage: { route: "r", version },
  };
}

/** A route with three versions, a journey on each, retired or not. */
function threeVersions(retired: boolean): RouteRead {
  const published = (version: number, journeys: string[]) => ({ version, published_at: `2026-10-0${String(version)}T00:00:00Z`, journeys });
  return {
    route: { header: { id: "r", name: "R", retired }, revision: 7, versions: [1, 2, 3] },
    detail: { header: { id: "r", name: "R", retired }, revision: 7, versions: [published(1, ["j_one"]), published(2, ["j_two"]), published(3, ["j_three"])], in_use: true },
    journeys: [summary("j_one", 1), summary("j_two", 2), summary("j_three", 3)],
  };
}

const listed = (read: RouteRead) => versionRows(read).map((row) => [row.version, row.journeys.map((journey) => journey.id)]);

describe("route detail (C17)", () => {
  it("lists the versions newest first, each with its journeys", () => {
    expect(listed(threeVersions(false))).toEqual([
      [3, ["j_three"]],
      [2, ["j_two"]],
      [1, ["j_one"]],
    ]);
  });

  it("lists a retired route's versions and journeys the same way (A19)", () => {
    expect(listed(threeVersions(true))).toEqual(listed(threeVersions(false)));
  });
});

describe("what an author can do to a route (A11, A19)", () => {
  it("opens a draft only when none is open, and publishes or discards only an open one", () => {
    const route = threeVersions(false).route;
    expect(routeMoves(route)).toEqual({ open: true, publish: false, discard: false, retire: true });
    expect(routeMoves({ ...route, draft: { extends: 3, graph: {} } })).toEqual({ open: false, publish: true, discard: true, retire: true });
    expect(routeMoves({ ...route, header: { ...route.header, retired: true } }).retire).toBe(false);
  });

  it("names an exported file by route and version, or draft", () => {
    expect(exportName("r", 2)).not.toBe(exportName("r", undefined));
    expect(exportName("r", 2)).toMatch(/\.yaml$/);
  });
});

describe("the routes index", () => {
  const route = (extra: Partial<Schema<"RouteSummary">["header"]>, latest: number | null): Schema<"RouteSummary"> => ({
    header: { id: "r", name: "R", ...extra },
    revision: 1,
    latest_version: latest,
    draft_open: false,
  });

  it("starts a journey from the latest published version of a route that is not retired, a draft being open or not", () => {
    expect(startableVersion(route({}, 3))).toBe(3);
    expect(startableVersion({ ...route({}, 3), draft_open: true })).toBe(3);
    expect(startableVersion(route({ retired: true }, 3))).toBeUndefined();
    expect(startableVersion(route({}, null))).toBeUndefined();
  });

  it("counts active journeys by the route they came from, and no others", () => {
    const journey = (status: Schema<"JourneySummary">["status"], route: string | null): Schema<"JourneySummary"> => ({
      ...summary("j", 1),
      status,
      lineage: route === null ? null : { route, version: 1 },
    });
    const counts = activeJourneyCounts([journey("active", "a"), journey("active", "a"), journey("completed", "a"), journey("active", "b"), journey("active", null)]);
    expect([counts.get("a"), counts.get("b"), counts.get("c")]).toEqual([2, 1, undefined]);
  });

  it("says a use in words that agree with its figures", () => {
    expect(inUseWords(0)).not.toBe(inUseWords(2));
    expect(inUseWords(2)).toContain("2");
  });
});
