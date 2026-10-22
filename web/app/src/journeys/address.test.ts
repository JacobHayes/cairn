// C16: the index's filters live in the address and become the host's query; "mine" asks for
// the journeys referring to the caller's identities, active only (B11), or nothing at all. A journey, a
// node and an edge each have one address; the view is its `view=`, written only off the default.
import { describe, expect, it } from "vitest";

import type { AddressName, Page } from "../address.ts";
import { viewStateFrom, walkthroughPath } from "./view.ts";
import {
  DEFAULT_FILTERS,
  SCREEN_WORDS,
  defaultScreen,
  edgeAddress,
  filterParams,
  filtersFrom,
  indexPath,
  journeyPath,
  layerEntry,
  layerExit,
  newJourneyPath,
  nextProjection,
  nodeAddress,
  queryOf,
  screenOf,
  screenSwitched,
  type IndexFilters,
  type JourneyTarget,
  type Screen,
} from "./address.ts";

const SCREENS = Object.keys(SCREEN_WORDS) as Screen[];

describe("the index's filters in the address", () => {
  const cases: [string, IndexFilters][] = [
    ["the default", DEFAULT_FILTERS],
    ["every status", { ...DEFAULT_FILTERS, status: "any" }],
    ["a route's version", { ...DEFAULT_FILTERS, route: "vendor-evaluation", version: 2 }],
    ["an owner", { ...DEFAULT_FILTERS, owner: "who_a" }],
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

  it("asks for the journeys an identity owns", () => {
    expect(queryOf({ ...DEFAULT_FILTERS, owner: "who_a" }, [])).toEqual({ status: ["active"], owner: ["who_a"] });
  });

  it("asks for the active journeys referring to the caller's identities for mine", () => {
    expect(queryOf({ ...DEFAULT_FILTERS, status: "any", mine: true }, ["who_a", "who_b"])).toEqual({
      status: ["active"],
      referencing: ["who_a", "who_b"],
    });
  });

  it("asks for nothing when mine can hold nothing: no identity, or a status that leaves mine", () => {
    expect(queryOf({ ...DEFAULT_FILTERS, mine: true }, [])).toBeUndefined();
    expect(queryOf({ ...DEFAULT_FILTERS, status: "completed", mine: true }, ["who_a"])).toBeUndefined();
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

  it.each<[string, JourneyTarget, Screen | undefined, string, string]>([
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
    for (const target of [JOURNEY, NODE]) {
      for (const screen of SCREENS) {
        const search = journeyPath(target, screen, "?depth=stages&q=x").split("?")[1] ?? "";
        expect(screenOf(target, new URLSearchParams(search))).toBe(screen);
      }
    }
    expect(screenOf(NODE, new URLSearchParams("view=bogus"))).toBe("graph");
  });

  it("shows a drawn line on the Graph, whatever view the query names", () => {
    for (const screen of SCREENS) {
      expect(screenOf(EDGE, new URLSearchParams(`view=${screen}`))).toBe("graph");
    }
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

/** The page the router matched: pattern `name` with `params`. */
const page = (name: AddressName, params: Record<string, string> = {}): Page => ({ name, params });
const J = { journey: "j_a" };

describe("opening and closing a node keeps the view", () => {
  it.each([
    ["opens a node on the list", page("journey", J), "?view=list", "n_x", "/journeys/j_a/nodes/n_x?view=list"],
    ["closes it back on the list", page("journeyNode", { ...J, key: "n_x" }), "?view=list", undefined, "/journeys/j_a?view=list"],
    ["opens a node on the Summary and keeps it there", page("journey", J), "", "n_x", "/journeys/j_a/nodes/n_x?view=summary"],
    ["closes it back to the bare journey", page("journeyNode", { ...J, key: "n_x" }), "?view=summary", undefined, "/journeys/j_a"],
    ["opens a node on the Graph with no view named", page("journey", J), "?view=graph&depth=stages", "n_x", "/journeys/j_a/nodes/n_x?depth=stages"],
    ["closes a node on the default Graph and names the Graph", page("journeyNode", { ...J, key: "n_x" }), "?depth=stages", undefined, "/journeys/j_a?view=graph&depth=stages"],
    ["closes an edge", page("journeyEdge", { ...J, edge: "n_x~n_y" }), "?lens=gravity", undefined, "/journeys/j_a?view=graph&lens=gravity"],
    ["opens a node from an edge", page("journeyEdge", { ...J, edge: "n_x~n_y" }), "", "n_y", "/journeys/j_a/nodes/n_y"],
    ["moves to the next node", page("journeyNode", { ...J, key: "n_x" }), "?view=timeline&range=3m", "n_z", "/journeys/j_a/nodes/n_z?view=timeline&range=3m"],
  ])("%s", (_, at, search, key, expected) => {
    expect(nodeAddress(at, search, key)).toBe(expected);
  });

  it("opens and closes a node of a route's draft or version, keeping its view", () => {
    expect(nodeAddress(page("route", { route: "r" }), "?view=list&depth=stages", "n_x")).toBe("/routes/r/nodes/n_x?view=list&depth=stages");
    expect(nodeAddress(page("routeNode", { route: "r", key: "n_x" }), "?view=timeline&q=a", undefined)).toBe("/routes/r?view=timeline&q=a");
    expect(nodeAddress(page("route", { route: "r" }), "?version=2&view=list", "n_x")).toBe("/routes/r/nodes/n_x?version=2&view=list");
  });

  it("takes a node off a proposal under any destination and adds one", () => {
    expect(nodeAddress(page("journeyProposalNode", { ...J, proposal: "p_1", key: "n_x" }), "", "n_y")).toBe("/journeys/j_a/proposals/p_1/nodes/n_y");
    expect(nodeAddress(page("routeProposal", { route: "r", proposal: "p_1" }), "?view=list", "n_y")).toBe("/routes/r/proposals/p_1/nodes/n_y?view=list");
    expect(nodeAddress(page("deploymentProposal", { proposal: "p_1" }), "", "n_x")).toBe("/identities/proposals/p_1");
  });

  it.each([
    ["a journey's opens it on the journey", page("journeyProposals", J), "n_x", "/journeys/j_a/nodes/n_x"],
    ["a route's opens it in the route's draft", page("routeProposals", { route: "r" }), "n_x", "/routes/r/nodes/n_x"],
    ["a journey's has none to close", page("journeyProposals", J), undefined, "/journeys/j_a/proposals"],
    ["a route's has none to close", page("routeProposals", { route: "r" }), undefined, "/routes/r/proposals"],
    ["the deployment's has no nodes", page("deploymentProposals"), "n_x", "/identities/proposals"],
  ])("a node from open proposals: %s", (_, at, key, expected) => {
    expect(nodeAddress(at, "", key)).toBe(expected);
  });
});

describe("opening a drawn line", () => {
  it.each([
    ["from a journey's List, on the Graph with the settings kept and no node", page("journeyNode", { ...J, key: "n_x" }), "?view=list&depth=stages", "/journeys/j_a/edges/n_x~n_y?depth=stages"],
    ["from the bare journey", page("journey", J), "?view=summary", "/journeys/j_a/edges/n_x~n_y"],
    ["from another line", page("journeyEdge", { ...J, edge: "n_a~n_b" }), "?lens=gravity", "/journeys/j_a/edges/n_x~n_y?lens=gravity"],
    ["from a route's List, keeping the version", page("routeNode", { route: "r", key: "n_a" }), "?version=2&view=list&depth=stages", "/routes/r/edges/n_x~n_y?version=2&depth=stages"],
    ["from a route's draft", page("route", { route: "r" }), "?q=a", "/routes/r/edges/n_x~n_y?q=a"],
  ])("writes the line's address %s", (_, at, search, expected) => {
    expect(edgeAddress(at, search, "n_x", "n_y")).toBe(expected);
  });

  it("closes into a node or the bare route, keeping the version", () => {
    const at = page("routeEdge", { route: "r", edge: "n_x~n_y" });
    expect(nodeAddress(at, "?version=2&view=list", "n_x")).toBe("/routes/r/nodes/n_x?version=2");
    expect(nodeAddress(at, "?version=2", undefined)).toBe("/routes/r?version=2");
    expect(nodeAddress(at, "", undefined)).toBe("/routes/r");
  });
});

describe("switching views", () => {
  const search = `?${new URLSearchParams({ depth: "decisions", frontier: "1", sort: "due", cols: "rank", range: "3m", open: "n_a" }).toString()}`;
  const paramsOf = (path: string) => new URLSearchParams(path.split("?")[1] ?? "");

  it("keeps the whole query, so switching back restores the other views' settings", () => {
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

describe("the proposal layer's addresses (C14)", () => {
  const J = { journey: "j_a" };

  it("opens on the Graph, and the Summary and the Walkthrough resolve to it under a layer", () => {
    expect(defaultScreen({ ...J, proposal: "p_1" })).toBe("graph");
    expect(screenOf({ ...J, proposal: "p_1" }, new URLSearchParams("view=summary"))).toBe("graph");
    expect(screenOf({ ...J, proposal: "p_1" }, new URLSearchParams("view=walk"))).toBe("graph");
    expect(screenOf({ ...J, proposal: "p_1" }, new URLSearchParams("view=timeline"))).toBe("timeline");
    expect(screenOf(J, new URLSearchParams("view=summary"))).toBe("summary");
  });

  it("enters the layer with the view on screen kept, and leaves it with the same view", () => {
    expect(layerEntry(J, "?view=list&depth=actions", "p_1")).toBe("/journeys/j_a/proposals/p_1?view=list&depth=actions");
    expect(layerExit(J, "?view=list&depth=actions")).toBe("/journeys/j_a?view=list&depth=actions");
  });

  it("enters from the Summary on the layer's Graph, and leaves to the journey's own front page", () => {
    expect(layerEntry(J, "", "p_1")).toBe("/journeys/j_a/proposals/p_1");
    expect(layerExit(J, "")).toBe("/journeys/j_a");
  });
});
