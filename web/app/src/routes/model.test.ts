// C17: route detail lists each version, newest first, with the journeys on it, and marks
// those with an upgrade available from each journey's own field (the host's). A retired route
// is hidden from new journeys only: its journeys still see upgrades (A19).
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import type { RouteRead } from "../data/reads.ts";
import { exportName, routeMoves, upgradable, versionRows } from "./model.ts";

type Summary = Schema<"JourneySummary">;

function summary(id: string, version: number, latest: number): Summary {
  return {
    id,
    name: id,
    status: "active",
    revision: 1,
    created_at: "2026-10-01T00:00:00Z",
    lineage: { route: "r", version },
    latest_version: latest,
    upgrade_available: version < latest,
  };
}

/** A route with three versions, a journey on each, retired or not. */
function threeVersions(retired: boolean): RouteRead {
  const published = (version: number, journeys: string[]) => ({ version, published_at: `2026-10-0${String(version)}T00:00:00Z`, journeys });
  return {
    route: { header: { id: "r", name: "R", retired }, revision: 7, versions: [1, 2, 3] },
    detail: { header: { id: "r", name: "R", retired }, revision: 7, versions: [published(1, ["j_one"]), published(2, ["j_two"]), published(3, ["j_three"])] },
    journeys: [summary("j_one", 1, 3), summary("j_two", 2, 3), summary("j_three", 3, 3)],
  };
}

const marks = (read: RouteRead) => versionRows(read).map((row) => [row.version, row.journeys.map((journey) => [journey.id, journey.upgrade])]);

describe("route detail (C17)", () => {
  it("lists the versions newest first, each with its journeys and their upgrade marks", () => {
    expect(marks(threeVersions(false))).toEqual([
      [3, [["j_three", false]]],
      [2, [["j_two", true]]],
      [1, [["j_one", true]]],
    ]);
  });

  it("still offers upgrades to the journeys of a retired route (A19)", () => {
    const read = threeVersions(true);
    expect(marks(read)).toEqual(marks(threeVersions(false)));
    expect(upgradable(versionRows(read)).map((journey) => journey.id).sort()).toEqual(["j_one", "j_two"]);
  });

  it("marks what the host says, never what the versions would suggest", () => {
    const read = threeVersions(false);
    read.journeys = read.journeys.map((each) => ({ ...each, upgrade_available: each.id === "j_three" }));
    expect(upgradable(versionRows(read)).map((journey) => journey.id)).toEqual(["j_three"]);
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
