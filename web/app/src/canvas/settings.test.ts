// The canvas's settings in the address: a graph's own settings ride beside the view state, a
// journey's and a route's.
import { describe, expect, it } from "vitest";

import { DEFAULT_VIEW_STATE, ROUTE_OFFERS } from "../journeys/view.ts";
import { screenOf } from "../journeys/address.ts";
import { DEFAULT_GRAPH_VIEW, graphPath, graphSearch, graphViewFrom, type GraphView } from "./settings.ts";

describe("a journey graph's address", () => {
  const query = (path: string) => new URLSearchParams(path.split("?")[1] ?? "");

  it("round-trips the view state and the graph's own settings, and names the graph only off its default", () => {
    const view: GraphView = { state: { ...DEFAULT_VIEW_STATE, depth: "deliverables", fade: true, frontier: true, text: "x" }, open: ["n_s"], shut: ["n_t"], lens: "gravity", edit: true };
    const bare = graphPath("j_a", view, {}, "");
    expect(graphViewFrom(query(bare))).toEqual(view);
    expect(screenOf({ journey: "j_a" }, query(bare))).toBe("graph");
    const onNode = graphPath("j_a", view, { node: "n_x" }, "");
    expect(onNode.startsWith("/journeys/j_a/nodes/n_x?")).toBe(true);
    expect(query(onNode).has("view")).toBe(false);
    expect(screenOf({ journey: "j_a", node: "n_x" }, query(onNode))).toBe("graph");
  });

  it("rewrites the graph's settings in the query it is given and keeps every other parameter, the other views' own settings included", () => {
    const path = graphPath("j_a", { ...DEFAULT_GRAPH_VIEW, open: ["n_s"] }, { node: "n_x" }, "?view=list&depth=stages&shut=n_t&sort=due&cols=kind&range=3m&frontier=1");
    const params = query(path);
    expect(graphViewFrom(params)).toEqual({ ...DEFAULT_GRAPH_VIEW, open: ["n_s"] });
    expect([params.get("sort"), params.get("cols"), params.get("range")]).toEqual(["due", "kind", "3m"]);
    expect(params.get("view")).toBeNull();
  });
});

describe("a route graph's address", () => {
  it("reads the view state as far as a route offers it, and the graph's own settings in full", () => {
    const params = new URLSearchParams("depth=deliverables&fade=1&mine=1&owner=u&flag=blocked&frontier=1&show=notrelevant&open=n_s&shut=n_t&lens=gravity");
    const view = graphViewFrom(params, ROUTE_OFFERS);
    expect(view.state).toEqual({ ...DEFAULT_VIEW_STATE, depth: "deliverables", fade: true, filter: { ...DEFAULT_VIEW_STATE.filter, conditional: false } });
    expect([view.open, view.shut, view.lens]).toEqual([["n_s"], ["n_t"], "gravity"]);
  });

  it("rewrites the graph's settings in a route's query and keeps the version and the view it is on", () => {
    const search = graphSearch({ ...DEFAULT_GRAPH_VIEW, open: ["n_s"] }, "?version=2&view=graph&depth=stages&shut=n_t&q=x");
    const params = new URLSearchParams(search);
    expect(params.get("open")).toBe("n_s");
    expect(params.has("shut")).toBe(false);
    expect([params.get("version"), params.get("view")]).toEqual(["2", "graph"]);
  });
});
