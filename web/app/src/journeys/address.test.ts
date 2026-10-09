// C16: the index's filters live in the address and become the host's query; "mine" asks for
// the journeys referring to the caller's entities, active only (B11), or nothing at all.
import { describe, expect, it } from "vitest";

import { paramsOf, revealing, viewFrom } from "../canvas/settings.ts";
import {
  DEFAULT_FILTERS,
  carriedSearch,
  deepLinkPath,
  filterParams,
  filtersFrom,
  indexPath,
  landingPath,
  legacyRedirect,
  nextProjection,
  pageOfPath,
  pagePath,
  projectionOf,
  queryOf,
  summaryPath,
  withParam,
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

describe("the journey's pages (2.2, 2.3)", () => {
  it("shows each page's projections in the one global order, leaving out the ones that do not apply", () => {
    expect(projectionOf("plan", "timeline")).toBe("timeline");
    expect(projectionOf("next", "timeline")).toBeUndefined();
    expect(projectionOf("plan", "cards")).toBeUndefined();
  });

  it("cycles forward with v within the page's own projections", () => {
    expect(nextProjection("plan", "graph")).toBe("list");
    expect(nextProjection("plan", "timeline")).toBe("graph");
    expect(nextProjection("next", "list")).toBe("cards");
    expect(nextProjection("next", "cards")).toBe("list");
  });

  it("builds the page's address with the node's detail before the query", () => {
    expect(pagePath("j_a", "plan", "graph")).toBe("/journeys/j_a/plan/graph");
    expect(pagePath("j_a", "next", "list", "n_x", "?mine=1")).toBe("/journeys/j_a/next/list/nodes/n_x?mine=1");
    expect(pagePath("j_a", "plan", "list", undefined, "q=a")).toBe("/journeys/j_a/plan/list?q=a");
    expect(summaryPath("j_a", "n_x")).toBe("/journeys/j_a/summary/nodes/n_x");
    expect(deepLinkPath("j_a", "n_x")).toBe("/journeys/j_a/nodes/n_x");
    expect(indexPath()).toBe("/journeys");
  });

  it("reads the page and projection back from an address, node or not", () => {
    expect(pageOfPath("/journeys/j_a/next/cards")).toEqual({ page: "next", projection: "cards" });
    expect(pageOfPath("/journeys/j_a/plan/timeline/nodes/n_x")).toEqual({ page: "plan", projection: "timeline" });
    expect(pageOfPath("/journeys/j_a/plan/cards")).toBeUndefined();
    expect(pageOfPath("/journeys/j_a/summary")).toBeUndefined();
  });

  it("sets a chip's parameter or removes it, keeping the rest", () => {
    expect(withParam("?mine=1", "decisions", "1")).toBe("?mine=1&decisions=1");
    expect(withParam("?decisions=1&q=a", "decisions", undefined)).toBe("?q=a");
    expect(withParam("?decisions=1", "decisions", undefined)).toBe("");
  });

  it("flips MINE whether the address says it as the chip or as the list's own flag", () => {
    expect(withMineFlipped("?q=a")).toBe("?q=a&mine=1");
    expect(withMineFlipped("?q=a&mine=1")).toBe("?q=a");
    expect(withMineFlipped("?flag=mine,overdue")).toBe("?flag=overdue");
    expect(withMineFlipped("?mine=1&flag=mine")).toBe("");
  });

  it("carries a toolbar chip only to the projections that read it", () => {
    const search = "?decisions=1&sort=due&mine=1&q=plan&kind=action";
    expect(carriedSearch(search, "plan", "list")).toBe("?decisions=1&mine=1&q=plan");
    expect(carriedSearch(search, "next", "cards")).toBe("?decisions=1&mine=1&q=plan");
    expect(carriedSearch(search, "plan", "graph")).toBe("?decisions=1");
    expect(carriedSearch(search, "plan", "timeline")).toBe("?decisions=1&q=plan");
    expect(carriedSearch("?sort=due", "plan", "list")).toBe("");
  });

  it("opens a journey with nothing recorded and a decision to make on the walkthrough (C11)", () => {
    expect(landingPath("j_a", { recorded: false, decisions: true })).toBe("/journeys/j_a/next/cards?decisions=1");
    expect(landingPath("j_a", { recorded: true, decisions: true })).toBe("/journeys/j_a/next/list");
    expect(landingPath("j_a", { recorded: false, decisions: false })).toBe("/journeys/j_a/next/list");
  });
});

describe("every old address resolves (2.4, Redirects)", () => {
  const rows: [string, string, string, string][] = [
    ["the old canvas with hide, in, notrelevant, undecided, heat", "/journeys/j_a", "?hide=action,milestone&in=n_stage&notrelevant=hide&undecided=hide&heat=on", "/journeys/j_a/plan/graph?kind=group%2Cdecision%2Cdeliverable&open=n_stage&show=&lens=gravity"],
    ["the old canvas hiding one kind", "/journeys/j_a", "?hide=action", "/journeys/j_a/plan/graph?kind=group%2Cdecision%2Cdeliverable%2Cmilestone"],
    ["the old canvas hiding only the undecided", "/journeys/j_a", "?undecided=hide", "/journeys/j_a/plan/graph?show=notrelevant"],
    ["the old canvas in edit mode", "/journeys/j_a", "?edit=on", "/journeys/j_a/plan/graph?edit=on"],
    ["the old canvas's trace, kept until selecting a node traces it", "/journeys/j_a/nodes/n_x", "?trace=on&hide=group", "/journeys/j_a/plan/graph/nodes/n_x?kind=decision%2Cdeliverable%2Caction%2Cmilestone&trace=on"],
    ["the overview", "/journeys/j_a/overview", "", "/journeys/j_a"],
    ["next", "/journeys/j_a/next", "", "/journeys/j_a/next/list"],
    ["next with its settings and a node", "/journeys/j_a/next/nodes/n_x", "?sort=due&mine=1", "/journeys/j_a/next/list/nodes/n_x?sort=due&mine=1"],
    ["list", "/journeys/j_a/list", "", "/journeys/j_a/plan/list"],
    ["list with its filters", "/journeys/j_a/list", "?flag=overdue&in=n_stage", "/journeys/j_a/plan/list?flag=overdue&in=n_stage"],
    ["triage", "/journeys/j_a/triage", "", "/journeys/j_a/next/cards"],
    ["triage in its decisions mode", "/journeys/j_a/triage", "?mode=decisions&mine=1", "/journeys/j_a/next/cards?mine=1&decisions=1"],
    ["the walkthrough", "/journeys/j_a/walkthrough", "", "/journeys/j_a/next/cards?decisions=1"],
    ["decisions", "/journeys/j_a/decisions", "", "/journeys/j_a/plan/graph?decisions=1"],
    ["a decision's detail on decisions", "/journeys/j_a/decisions/nodes/n_x", "", "/journeys/j_a/plan/graph/nodes/n_x?decisions=1"],
    ["decisions with the canvas's settings", "/journeys/j_a/decisions", "?hide=group", "/journeys/j_a/plan/graph?kind=decision%2Cdeliverable%2Caction%2Cmilestone&decisions=1"],
    ["the timeline", "/journeys/j_a/timeline", "", "/journeys/j_a/plan/timeline"],
    ["a node on the timeline, leaving the canvas's settings behind", "/journeys/j_a/timeline/nodes/n_x", "?hide=action&heat=on", "/journeys/j_a/plan/timeline/nodes/n_x"],
    ["the routes index", "/routes", "", "/library?type=routes"],
    ["the journey index, which was at the root with its filters", "/", "?status=archived&route=r&version=2&mine=1", "/journeys?status=archived&route=r&version=2&mine=1"],
  ];
  it.each(rows)("%s", (_, pathname, search, expected) => {
    expect(legacyRedirect(pathname, search)).toBe(expected);
  });

  it("leaves the summary, the landing and the deep link, and every new address alone", () => {
    for (const [pathname, search] of [
      ["/journeys/j_a/summary", ""],
      ["/journeys/j_a/summary/nodes/n_x", ""],
      ["/journeys/j_a", ""],
      ["/journeys/j_a/nodes/n_x", ""],
      ["/", ""],
      ["/journeys/j_a/next/list", "?mine=1"],
      ["/journeys/j_a/plan/graph/nodes/n_x", "?kind=action"],
      ["/routes/r/versions", ""],
    ] as const) {
      expect(legacyRedirect(pathname, search)).toBeUndefined();
    }
  });

  it("lands on a canvas that reads the same view the old address named", () => {
    const view = viewFrom(new URLSearchParams(legacyRedirect("/journeys/j_a", "?hide=action&notrelevant=hide&heat=on")?.split("?")[1]));
    expect(view.shown).toEqual(["group", "decision", "deliverable", "milestone"]);
    expect([view.notRelevant, view.undecided, view.heat]).toEqual([false, true, true]);
  });
});

describe("the canvas's settings in the address", () => {
  it("round-trips every setting and leaves the toolbar's chips alone", () => {
    const params = new URLSearchParams("decisions=1&kind=action,milestone&open=n_s&show=conditional&lens=gravity&trace=on&edit=on&q=x");
    const view = viewFrom(params);
    expect(view.rest).toEqual([["decisions", "1"], ["q", "x"]]);
    expect(Object.fromEntries(paramsOf(view))).toEqual(Object.fromEntries(params));
  });

  it("reveals a found node by showing what the view hides of it, and only that", () => {
    const view = viewFrom(new URLSearchParams("kind=group&show=conditional&decisions=1&q=x"));
    const found = revealing(view, { kind: "action", parent: "n_plan" }, "not_relevant");
    expect(Object.fromEntries(paramsOf(found))).toEqual({ kind: "group,action", open: "n_plan", trace: "on", q: "x" });
    const decision = revealing(viewFrom(new URLSearchParams("show=notrelevant")), { kind: "decision" }, "conditional");
    expect([decision.notRelevant, decision.undecided]).toEqual([true, true]);
    expect(revealing(viewFrom(new URLSearchParams("show=")), { kind: "group" }, "ready").notRelevant).toBe(false);
  });

  it("is bare at the defaults, and a kind list that is empty shows no kind", () => {
    expect(paramsOf(viewFrom(new URLSearchParams())).toString()).toBe("");
    expect(viewFrom(new URLSearchParams("kind=")).shown).toEqual([]);
    expect(viewFrom(new URLSearchParams("show=")).notRelevant).toBe(false);
  });
});
