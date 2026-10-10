// C16: the index's filters live in the address and become the host's query; "mine" asks for
// the journeys referring to the caller's entities, active only (B11), or nothing at all.
import { describe, expect, it } from "vitest";

import { paramsOf, revealing, viewFrom } from "../canvas/settings.ts";
import { screenPath } from "../detail/parts.tsx";
import {
  DEFAULT_FILTERS,
  carriedSearch,
  filterParams,
  filtersFrom,
  legacyRedirect,
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

  it("carries a toolbar chip only to the views that read it", () => {
    const search = "?decisions=1&frontier=1&sort=due&mine=1&q=plan&kind=action";
    expect(carriedSearch(search, "list")).toBe("?decisions=1&mine=1&q=plan&frontier=1");
    expect(carriedSearch(search, "graph")).toBe("?decisions=1&frontier=1");
    expect(carriedSearch(search, "walk")).toBe("?decisions=1&mine=1&q=plan");
    expect(carriedSearch(search, "summary")).toBe("");
  });

  it("closes a node's detail to the screen it opened on (C12)", () => {
    expect(screenPath("/journeys/j_a/graph/nodes/n_x")).toBe("/journeys/j_a/graph");
    expect(screenPath("/journeys/j_a/graph/edges/n_x~n_y")).toBe("/journeys/j_a/graph");
    expect(screenPath("/journeys/j_a/summary")).toBe("/journeys/j_a/summary");
  });
});

describe("every old address resolves (2.4, Redirects)", () => {
  const rows: [string, string, string, string][] = [
    ["the old canvas with hide, in, notrelevant, undecided, heat", "/journeys/j_a", "?hide=action,milestone&in=n_stage&notrelevant=hide&undecided=hide&heat=on", "/journeys/j_a/graph?kind=group%2Cdecision%2Cdeliverable&open=n_stage&show=&lens=gravity"],
    ["the next list, now the list with the Frontier on, with its settings and a node", "/journeys/j_a/next/list/nodes/n_x", "?sort=due&mine=1", "/journeys/j_a/list/nodes/n_x?sort=due&mine=1&frontier=1"],
    ["the cards, now the walk through", "/journeys/j_a/next/cards", "?decisions=1", "/journeys/j_a/walk?decisions=1"],
    ["a plan view, now the view", "/journeys/j_a/plan/timeline", "?q=x", "/journeys/j_a/timeline?q=x"],
    ["a plan graph's edge", "/journeys/j_a/plan/graph/edges/n_x~n_y", "", "/journeys/j_a/graph/edges/n_x~n_y"],
    ["the bare plan", "/journeys/j_a/plan", "", "/journeys/j_a/graph"],
    ["the decision walkthrough of the earlier screens", "/journeys/j_a/walkthrough", "", "/journeys/j_a/walk?decisions=1"],
    ["a node's address with no key, which is the view without it", "/journeys/j_a/graph/nodes", "?kind=action", "/journeys/j_a/graph?kind=action"],
    ["an old page's node address with no key", "/journeys/j_a/plan/list/nodes", "", "/journeys/j_a/plan/list"],
    ["a journey's node address with no key", "/journeys/j_a/nodes", "", "/journeys/j_a"],
    ["the journey index, which was at the root with its filters", "/", "?status=archived&route=r&version=2&mine=1", "/journeys?status=archived&route=r&version=2&mine=1"],
  ];
  it.each(rows)("%s", (_, pathname, search, expected) => {
    expect(legacyRedirect(pathname, search)).toBe(expected);
  });

  it("leaves the new views, the landing and an unknown path alone", () => {
    for (const [pathname, search] of [
      ["/journeys/j_a/summary", ""],
      ["/journeys/j_a/list", "?frontier=1"],
      ["/", ""],
      ["/journeys/j_a/graph/nodes/n_x", "?kind=action"],
      ["/journeys/j_a/nowhere", ""],
    ] as const) {
      expect(legacyRedirect(pathname, search)).toBeUndefined();
    }
  });
});

describe("the canvas's settings in the address", () => {
  it("round-trips every setting and leaves the toolbar's chips alone", () => {
    const params = new URLSearchParams("decisions=1&detail=work&open=n_s&shut=n_t&kind=action,milestone&show=notrelevant,conditional&lens=gravity&origins=1&edit=on&select=1&q=x");
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
