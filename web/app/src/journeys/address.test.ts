// C16: the index's filters live in the address and become the host's query; "mine" asks for
// the journeys referring to the caller's entities, active only (B11), or nothing at all. A journey, a
// node and an edge each have one address; the view is its `view=`, written only off the default.
import { describe, expect, it } from "vitest";

import { viewStateFrom } from "./view.ts";
import {
  DEFAULT_FILTERS,
  SCREENS,
  defaultScreen,
  filterParams,
  filtersFrom,
  indexPath,
  journeyPath,
  newJourneyPath,
  nextProjection,
  nodeOpened,
  queryOf,
  screenOf,
  screenSwitched,
  walkthroughPath,
  type IndexFilters,
  type JourneyTarget,
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
});

const JOURNEY: JourneyTarget = { journey: "j_a" };
const NODE: JourneyTarget = { journey: "j_a", node: "n_x" };
const EDGE: JourneyTarget = { journey: "j_a", edge: "n_x~n_y" };

describe("a journey's address", () => {
  it("names the Summary for a bare journey and the Graph for a node or an edge", () => {
    expect([defaultScreen(JOURNEY), defaultScreen(NODE), defaultScreen(EDGE)]).toEqual(["summary", "graph", "graph"]);
  });

  it.each<[string, JourneyTarget, (typeof SCREENS)[number] | undefined, string, string]>([
    ["the default of a bare journey", JOURNEY, "summary", "", "/journeys/j_a"],
    ["another view of a bare journey, view first", JOURNEY, "graph", "?depth=stages", "/journeys/j_a?view=graph&depth=stages"],
    ["the default of a node", NODE, "graph", "?depth=stages", "/journeys/j_a/nodes/n_x?depth=stages"],
    ["another view of a node", NODE, "summary", "", "/journeys/j_a/nodes/n_x?view=summary"],
    ["an edge in the graph", EDGE, "graph", "", "/journeys/j_a/edges/n_x~n_y"],
    ["a view in the query is replaced", NODE, "list", "?view=walk&q=x", "/journeys/j_a/nodes/n_x?view=list&q=x"],
    ["the query without its question mark", JOURNEY, undefined, "frontier=1", "/journeys/j_a?frontier=1"],
  ])("writes %s", (_, target, screen, search, path) => {
    expect(screen === undefined ? journeyPath(target, undefined, search) : journeyPath(target, screen, search)).toBe(path);
  });

  it("reads the view back from the query it was written to, whatever order the query is in", () => {
    for (const target of [JOURNEY, NODE, EDGE]) {
      for (const screen of SCREENS) {
        const search = journeyPath(target, screen, "?depth=stages&q=x").split("?")[1] ?? "";
        expect(screenOf(target, new URLSearchParams(search))).toBe(screen);
      }
    }
    expect(screenOf(NODE, new URLSearchParams("view=bogus"))).toBe("graph");
  });

  it("opens a node from a slash-ended journey address as from the bare one", () => {
    expect(nodeOpened("/journeys/j_a/", "", "n_x")).toBe(nodeOpened("/journeys/j_a", "", "n_x"));
    expect(nodeOpened("/journeys/j_a/", "", "n_x")).toBe("/journeys/j_a/nodes/n_x?view=summary");
  });

  it("gives the index, the new-journey form and the walkthrough addresses the router names", () => {
    expect(indexPath().startsWith("/journeys")).toBe(true);
    expect(indexPath({ ...DEFAULT_FILTERS, mine: true })).toBe("/journeys?mine=1");
    expect(newJourneyPath("vendor", 2)).toBe("/journeys/new?route=vendor&version=2");
    const [path = "", query = ""] = walkthroughPath("j_new").split("?");
    expect(path).toBe("/journeys/j_new");
    expect(screenOf({ journey: "j_new" }, new URLSearchParams(query))).toBe("walk");
    expect(viewStateFrom(new URLSearchParams(query)).depth).toBe("decisions");
  });
});

describe("opening and closing a node keeps the view", () => {
  it.each([
    ["opens a node on the list", "/journeys/j_a", "?view=list", "n_x", "/journeys/j_a/nodes/n_x?view=list"],
    ["closes it back on the list", "/journeys/j_a/nodes/n_x", "?view=list", undefined, "/journeys/j_a?view=list"],
    ["opens a node on the Summary and keeps it there", "/journeys/j_a", "", "n_x", "/journeys/j_a/nodes/n_x?view=summary"],
    ["closes it back to the bare journey", "/journeys/j_a/nodes/n_x", "?view=summary", undefined, "/journeys/j_a"],
    ["opens a node on the Graph with no view named", "/journeys/j_a", "?view=graph&depth=stages", "n_x", "/journeys/j_a/nodes/n_x?depth=stages"],
    ["closes a node on the default Graph and names the Graph", "/journeys/j_a/nodes/n_x", "?depth=stages", undefined, "/journeys/j_a?view=graph&depth=stages"],
    ["closes an edge", "/journeys/j_a/edges/n_x~n_y", "?lens=gravity", undefined, "/journeys/j_a?view=graph&lens=gravity"],
    ["opens a node from an edge", "/journeys/j_a/edges/n_x~n_y", "", "n_y", "/journeys/j_a/nodes/n_y"],
    ["moves to the next node", "/journeys/j_a/nodes/n_x", "?view=timeline&range=3m", "n_z", "/journeys/j_a/nodes/n_z?view=timeline&range=3m"],
  ])("%s", (_, pathname, search, key, expected) => {
    expect(nodeOpened(pathname, search, key)).toBe(expected);
  });

  it("takes a node off a route's draft or a proposal and adds one", () => {
    expect(nodeOpened("/routes/r/draft", "?edit=on", "n_x")).toBe("/routes/r/draft/nodes/n_x?edit=on");
    expect(nodeOpened("/routes/r/draft/nodes/n_x", "", undefined)).toBe("/routes/r/draft");
    expect(nodeOpened("/proposals/p_1/nodes/n_x", "", "n_y")).toBe("/proposals/p_1/nodes/n_y");
  });
});

describe("switching views", () => {
  const search = `?${new URLSearchParams({ depth: "decisions", frontier: "1", sort: "due", cols: "rank", range: "3m", open: "n_a", map: "1" }).toString()}`;
  const paramsOf = (path: string) => new URLSearchParams(path.split("?")[1] ?? "");

  it("keeps the whole query but the full-screen map, so switching back restores the other views' settings", () => {
    const to = paramsOf(screenSwitched(JOURNEY, `${search}&view=graph`, "list"));
    expect(Object.fromEntries(to)).toEqual({ view: "list", depth: "decisions", frontier: "1", sort: "due", cols: "rank", range: "3m", open: "n_a" });
    const back = paramsOf(screenSwitched(JOURNEY, `?${to.toString()}`, "graph"));
    expect(Object.fromEntries(back)).toEqual({ view: "graph", depth: "decisions", frontier: "1", sort: "due", cols: "rank", range: "3m", open: "n_a" });
  });

  it("keeps the node, except into the Walkthrough from another view, and takes the one it is given", () => {
    expect(screenSwitched(NODE, "", "list")).toBe("/journeys/j_a/nodes/n_x?view=list");
    expect(screenSwitched(NODE, "?view=list", "walk")).toBe("/journeys/j_a?view=walk");
    expect(screenSwitched(NODE, "?view=walk", "graph")).toBe("/journeys/j_a/nodes/n_x");
    expect(screenSwitched(NODE, "?view=walk", "list")).toBe("/journeys/j_a/nodes/n_x?view=list");
    expect(screenSwitched(JOURNEY, "?view=list", "graph", "n_y")).toBe("/journeys/j_a/nodes/n_y");
    expect(screenSwitched(NODE, "", "summary")).toBe("/journeys/j_a/nodes/n_x?view=summary");
  });

  it("keeps an edge only in the graph, and the view already open as it is", () => {
    expect(screenSwitched(EDGE, "", "list")).toBe("/journeys/j_a?view=list");
    expect(screenSwitched(EDGE, "?view=list", "graph")).toBe("/journeys/j_a/edges/n_x~n_y");
    expect(screenSwitched(JOURNEY, search, "summary")).toBe(`/journeys/j_a${search}`);
  });
});
