// The canvas's settings in the address: a journey graph's own settings ride beside the view state,
// a route canvas keeps its own, and opening a node reveals what the view hides of it (C2, 5.7).
import { describe, expect, it } from "vitest";

import { DEFAULT_VIEW_STATE, viewStateFrom } from "../journeys/view.ts";
import { canvasPath, DEFAULT_GRAPH_VIEW, graphViewFrom, paramsOf, revealing, viewFrom, type GraphView } from "./settings.ts";

describe("a journey graph's address", () => {
  it("round-trips the view state and the graph's own settings", () => {
    const view: GraphView = { state: { ...DEFAULT_VIEW_STATE, depth: "deliverables", fade: true, frontier: true, text: "x" }, open: ["n_s"], shut: ["n_t"], lens: "gravity", edit: true, map: true };
    const [, query = ""] = canvasPath("j_a", view).split("?");
    expect(graphViewFrom(new URLSearchParams(query))).toEqual(view);
    expect(canvasPath("j_a", DEFAULT_GRAPH_VIEW)).toBe("/journeys/j_a/graph");
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

describe("revealing a node (5.7)", () => {
  const narrowed: GraphView = {
    ...DEFAULT_GRAPH_VIEW,
    state: { depth: "stages", fade: true, frontier: true, filter: { mine: true, owner: "e_a", flags: ["stale"], notRelevant: false, conditional: false }, text: "x" },
    shut: ["n_stage"],
  };

  it("draws the node from a narrowed view: its depth, its class, its containers open, no narrowing; Frontier and Fade, which hide nothing, stay", () => {
    const found = revealing(narrowed, { kind: "action", ancestors: ["n_plan", "n_stage"], top: "group" }, "not_relevant");
    expect(found.state.depth).toBe("actions");
    expect(found.state.filter).toEqual({ mine: false, owner: undefined, flags: [], notRelevant: true, conditional: false });
    expect(found.state.text).toBe("");
    expect(found.open).toEqual(["n_plan", "n_stage"]);
    expect(found.shut).toEqual([]);
    expect([found.state.frontier, found.state.fade]).toEqual([true, true]);
  });

  it("keeps the depth for a decision the depth already draws", () => {
    const found = revealing({ ...narrowed, state: { ...narrowed.state, depth: "decisions" } }, { kind: "decision", ancestors: [], top: "decision" }, "conditional");
    expect([found.state.depth, found.state.filter.conditional]).toEqual(["decisions", true]);
    expect(viewStateFrom(new URLSearchParams(canvasPath("j_a", found).split("?")[1])).filter.conditional).toBe(true);
  });

  it("moves off Stages for a deliverable under a top-level decision, which Stages' top-level rule leaves out, though Stages draws the kind", () => {
    const found = revealing(narrowed, { kind: "deliverable", ancestors: ["n_ask"], top: "decision" }, undefined);
    expect(found.state.depth).toBe("deliverables");
    expect(revealing(narrowed, { kind: "deliverable", ancestors: ["n_stage"], top: "group" }, undefined).state.depth).toBe("stages");
  });

  it("moves off Stages for a new top-level decision, to the first depth that draws it", () => {
    const found = revealing(narrowed, { kind: "decision", ancestors: [], top: "decision" }, undefined);
    expect(found.state.depth).toBe("decisions");
  });
});
