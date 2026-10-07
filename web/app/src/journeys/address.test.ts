// C16: the index's filters live in the address and become the host's query; "mine" asks for
// the journeys referring to the caller's entities, active only (B11), or nothing at all.
import { describe, expect, it } from "vitest";

import { DEFAULT_FILTERS, filterParams, filtersFrom, queryOf, type IndexFilters } from "./address.ts";

describe("the index's filters in the address", () => {
  const cases: [string, IndexFilters][] = [
    ["the default", DEFAULT_FILTERS],
    ["every status", { ...DEFAULT_FILTERS, status: "any" }],
    ["a route's version", { ...DEFAULT_FILTERS, route: "vendor-evaluation", version: 2 }],
    ["mine with an upgrade", { ...DEFAULT_FILTERS, status: "archived", mine: true, upgrade: true }],
  ];
  it.each(cases)("round-trip: %s", (_, filters) => {
    expect(filtersFrom(filterParams(filters))).toEqual(filters);
  });

  it("leaves the default bare, and drops a version without its route or one that is no version", () => {
    expect(filterParams(DEFAULT_FILTERS).toString()).toBe("");
    expect(filtersFrom(new URLSearchParams("version=2")).version).toBeUndefined();
    expect(filtersFrom(new URLSearchParams("route=r&version=0")).version).toBeUndefined();
    expect(filtersFrom(new URLSearchParams("status=paused")).status).toBe("active");
  });
});

describe("the host's query for the filters", () => {
  it("names the status, lineage, and upgrade filters as the host reads them", () => {
    expect(queryOf({ ...DEFAULT_FILTERS, route: "r", version: 3, upgrade: true }, [])).toEqual({
      status: ["active"],
      route: "r",
      version: 3,
      upgrade_available: true,
    });
    expect(queryOf({ ...DEFAULT_FILTERS, status: "any" }, [])).toEqual({});
  });

  it("asks for the active journeys referring to the caller's entities for mine", () => {
    expect(queryOf({ ...DEFAULT_FILTERS, status: "any", mine: true }, ["e_a", "e_b"])).toEqual({
      status: ["active"],
      referencing: ["e_a", "e_b"],
    });
  });

  it("asks for nothing when mine can hold nothing: no entity, or a status that leaves mine", () => {
    expect(queryOf({ ...DEFAULT_FILTERS, mine: true }, [])).toBeUndefined();
    expect(queryOf({ ...DEFAULT_FILTERS, status: "completed", mine: true }, ["e_a"])).toBeUndefined();
  });
});
