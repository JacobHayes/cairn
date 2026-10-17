// The canvas's settings in the address: a journey graph's own settings ride beside the view state
// and a route canvas keeps its own.
import { describe, expect, it } from "vitest";

import { DEFAULT_VIEW_STATE } from "../journeys/view.ts";
import { screenOf } from "../journeys/address.ts";
import { DEFAULT_GRAPH_VIEW, graphPath, graphViewFrom, paramsOf, viewFrom, type GraphView } from "./settings.ts";

describe("a journey graph's address", () => {
  const query = (path: string) => new URLSearchParams(path.split("?")[1] ?? "");

  it("round-trips the view state and the graph's own settings, and names the graph only off its default", () => {
    const view: GraphView = { state: { ...DEFAULT_VIEW_STATE, depth: "deliverables", fade: true, frontier: true, text: "x" }, open: ["n_s"], shut: ["n_t"], lens: "gravity", edit: true, map: true };
    const bare = graphPath("j_a", view, {}, "");
    expect(graphViewFrom(query(bare))).toEqual(view);
    expect(screenOf({ journey: "j_a" }, query(bare))).toBe("graph");
    const onNode = graphPath("j_a", view, { node: "n_x" }, "");
    expect(onNode.startsWith("/journeys/j_a/nodes/n_x?")).toBe(true);
    expect(query(onNode).has("view")).toBe(false);
    expect(screenOf({ journey: "j_a", node: "n_x" }, query(onNode))).toBe("graph");
    expect(graphPath("j_a", DEFAULT_GRAPH_VIEW, { edge: "n_a~n_b" }, "")).toBe("/journeys/j_a/edges/n_a~n_b");
  });

  it("rewrites the graph's settings in the query it is given and keeps every other parameter, the other views' own settings included", () => {
    const path = graphPath("j_a", { ...DEFAULT_GRAPH_VIEW, open: ["n_s"] }, { node: "n_x" }, "?view=list&depth=stages&shut=n_t&sort=due&cols=kind&range=3m&frontier=1");
    const params = query(path);
    expect(graphViewFrom(params)).toEqual({ ...DEFAULT_GRAPH_VIEW, open: ["n_s"] });
    expect([params.get("sort"), params.get("cols"), params.get("range")]).toEqual(["due", "kind", "3m"]);
    expect(params.get("view")).toBeNull();
  });
});

describe("a route canvas's address", () => {
  it("round-trips its settings and keeps the parameters it does not own", () => {
    const params = new URLSearchParams("depth=deliverables&open=n_s&shut=n_t&in=n_c&lens=gravity&edit=on&view=list");
    const view = viewFrom(params);
    expect(view.rest).toEqual([["view", "list"]]);
    expect(Object.fromEntries(paramsOf(view))).toEqual(Object.fromEntries(params));
  });
});
