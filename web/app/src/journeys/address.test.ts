// C16: the index's filters live in the address and become the host's query; "mine" asks for
// the journeys referring to the caller's entities, active only (B11), or nothing at all.
import { describe, expect, it } from "vitest";

import { paramsOf, revealing, viewFrom } from "../canvas/settings.ts";
import { screenPath } from "../detail/parts.tsx";
import {
  DEFAULT_FILTERS,
  switchedSearch,
  filterParams,
  filtersFrom,
  nextProjection,
  queryOf,
  withMineFlipped,
  type IndexFilters,
} from "./address.ts";

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

describe("the journey's views (2.2, 2.3)", () => {
  it("cycles forward with v through the projections", () => {
    expect(nextProjection("graph")).toBe("list");
    expect(nextProjection("timeline")).toBe("graph");
  });

  it("flips MINE whether the address says it as the chip or as the list's own flag", () => {
    expect(withMineFlipped("?q=a")).toBe("?q=a&mine=1");
    expect(withMineFlipped("?q=a&mine=1")).toBe("?q=a");
    expect(withMineFlipped("?flag=mine,overdue")).toBe("?flag=overdue");
    expect(withMineFlipped("?mine=1&flag=mine")).toBe("");
  });

  it("carries the shared controls to every view, so a trip through one that ignores them keeps them", () => {
    const search = "?decisions=1&frontier=1&sort=due&mine=1&q=plan&kind=action";
    const carried = "?decisions=1&mine=1&q=plan&frontier=1";
    for (const to of ["timeline", "graph", "walk", "summary"] as const) {
      expect(switchedSearch(search, "list", to)).toBe(carried);
    }
    expect(switchedSearch(switchedSearch(search, "list", "summary"), "summary", "graph")).toBe(carried);
  });

  it("carries the Filter's not-relevant switch between the graph and the list, keeping the graph's conditional nodes drawn", () => {
    expect(switchedSearch("?show=notrelevant,conditional&frontier=1", "graph", "list")).toBe("?frontier=1&show=notrelevant");
    expect(switchedSearch("?show=notrelevant", "list", "graph")).toBe("?show=notrelevant%2Cconditional");
    expect(switchedSearch("?show=conditional", "graph", "list")).toBe("");
    expect(switchedSearch("?show=notrelevant", "list", "timeline")).toBe("?show=notrelevant");
  });

  it("opens a view with the chips only: its own settings are the stock ones, and the screen already open keeps its query", () => {
    expect(switchedSearch("?sort=due&open=n_setup&decisions=1", "list", "graph")).toBe("?decisions=1");
    expect(switchedSearch("?flag=mine", "list", "timeline")).toBe("?mine=1");
    expect(switchedSearch("?sort=due", "list", "list")).toBe("?sort=due");
  });

  it("closes a node's detail to the screen it opened on (C12)", () => {
    expect(screenPath("/journeys/j_a/graph/nodes/n_x")).toBe("/journeys/j_a/graph");
    expect(screenPath("/journeys/j_a/graph/edges/n_x~n_y")).toBe("/journeys/j_a/graph");
    expect(screenPath("/journeys/j_a/summary")).toBe("/journeys/j_a/summary");
    expect(screenPath("/mine/j_a/nodes/n_x")).toBe("/mine");
  });
});

describe("the canvas's settings in the address", () => {
  it("round-trips every setting and leaves the toolbar's chips alone", () => {
    const params = new URLSearchParams("decisions=1&detail=work&open=n_s&shut=n_t&kind=action,milestone&show=notrelevant,conditional&lens=gravity&edit=on&q=x");
    const view = viewFrom(params);
    expect(view.rest).toEqual([["decisions", "1"], ["q", "x"]]);
    expect(Object.fromEntries(paramsOf(view))).toEqual(Object.fromEntries(params));
    expect(paramsOf(viewFrom(new URLSearchParams())).toString()).toBe("");
    expect(viewFrom(new URLSearchParams("kind=")).shown).toEqual([]);
  });

  it("reveals a found node by showing what the view hides of it, and only that", () => {
    const view = viewFrom(new URLSearchParams("kind=group&show=conditional&decisions=1&q=x"));
    const found = revealing(view, { kind: "action", ancestors: ["n_plan", "n_stage"] }, "not_relevant");
    expect(Object.fromEntries(paramsOf(found))).toEqual({ detail: "all", kind: "group,action", open: "n_plan,n_stage", show: "notrelevant,conditional", q: "x" });
    const decision = revealing(viewFrom(new URLSearchParams("show=notrelevant")), { kind: "decision", ancestors: [] }, "conditional");
    expect([decision.notRelevant, decision.undecided]).toEqual([true, true]);
    expect(revealing(viewFrom(new URLSearchParams("show=")), { kind: "group", ancestors: [] }, "ready").notRelevant).toBe(false);
  });
});
