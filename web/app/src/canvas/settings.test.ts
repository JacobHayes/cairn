// The canvas's settings in the address: a journey graph's own settings ride beside the view state,
// a route canvas keeps its own, and opening a node reveals what the view hides of it (C2, 5.7).
import { describe, expect, it } from "vitest";

import { DEFAULT_VIEW_STATE, viewStateFrom } from "../journeys/view.ts";
import { canvasPath, DEFAULT_GRAPH_VIEW, graphViewFrom, paramsOf, revealing, viewFrom, type GraphView } from "./settings.ts";

describe("a journey graph's address", () => {
  it("round-trips the view state and the graph's own settings", () => {
    const view: GraphView = { state: { ...DEFAULT_VIEW_STATE, step: "work", frontier: true, text: "x" }, open: ["n_s"], shut: ["n_t"], lens: "gravity", edit: true, map: true };
    const [, query = ""] = canvasPath("j_a", view).split("?");
    expect(graphViewFrom(new URLSearchParams(query))).toEqual(view);
    expect(canvasPath("j_a", DEFAULT_GRAPH_VIEW)).toBe("/journeys/j_a/graph");
  });
});

describe("a route canvas's address", () => {
  it("round-trips its settings and keeps the parameters it does not own", () => {
    const params = new URLSearchParams("detail=work&open=n_s&shut=n_t&in=n_c&lens=gravity&edit=on&view=list");
    const view = viewFrom(params);
    expect(view.rest).toEqual([["view", "list"]]);
    expect(Object.fromEntries(paramsOf(view))).toEqual(Object.fromEntries(params));
  });
});

describe("revealing a node (5.7)", () => {
  const narrowed: GraphView = {
    ...DEFAULT_GRAPH_VIEW,
    state: { step: "stages", frontier: true, decisions: true, filter: { mine: true, owner: "e_a", flags: ["stale"], notRelevant: false, conditional: false }, text: "x" },
    shut: ["n_stage"],
  };

  it("draws the node from a narrowed view: its step, its class, its containers open, no narrowing; the highlights it does not hide stay", () => {
    const found = revealing(narrowed, { kind: "action", ancestors: ["n_plan", "n_stage"], top: "group" }, "not_relevant");
    expect(found.state.step).toBe("all");
    expect(found.state.filter).toEqual({ mine: false, owner: undefined, flags: [], notRelevant: true, conditional: false });
    expect(found.state.text).toBe("");
    expect(found.open).toEqual(["n_plan", "n_stage"]);
    expect(found.shut).toEqual([]);
    expect([found.state.frontier, found.state.decisions]).toEqual([true, false]);
  });

  it("keeps Decisions and the step for a decision the step already draws", () => {
    const found = revealing({ ...narrowed, state: { ...narrowed.state, step: "decisions" } }, { kind: "decision", ancestors: [], top: "decision" }, "conditional");
    expect([found.state.step, found.state.decisions, found.state.filter.conditional]).toEqual(["decisions", true, true]);
    expect(viewStateFrom(new URLSearchParams(canvasPath("j_a", found).split("?")[1])).filter.conditional).toBe(true);
  });

  it("moves off Stages for a deliverable under a top-level decision, which Stages' top-level rule leaves out, though Stages draws the kind", () => {
    const found = revealing(narrowed, { kind: "deliverable", ancestors: ["n_ask"], top: "decision" }, undefined);
    expect(found.state.step).toBe("work");
    expect(revealing(narrowed, { kind: "deliverable", ancestors: ["n_stage"], top: "group" }, undefined).state.step).toBe("stages");
  });

  it("moves off Stages for a new top-level decision, and keeps Decisions on for it", () => {
    const found = revealing(narrowed, { kind: "decision", ancestors: [], top: "decision" }, undefined);
    expect([found.state.step, found.state.decisions]).toEqual(["decisions", true]);
  });
});
