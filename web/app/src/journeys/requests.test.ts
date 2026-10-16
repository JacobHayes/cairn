// C2, C9, C10, C11, C13: each shared setting reaches every view's requests. Detail, the Filter
// and the search are in the one level request every view draws from; the list's table and the
// walk through read their rows beside it; Frontier and Decisions light nodes and never change
// what is drawn or how it is laid out.
import { describe, expect, it } from "vitest";

import { actingView } from "../acting/acting.test-support.ts";
import { badgesOf, journeyLayoutView, levelRequestOf, lightsUp, litOf, stepHides, tableQueryOf, walkQueryOf } from "./requests.ts";
import { DEFAULT_FILTER, DEFAULT_VIEW_STATE, type ViewState } from "./view.ts";

const view = actingView();
const nodes = view.journey.graph.nodes ?? [];

const narrowed: ViewState = {
  ...DEFAULT_VIEW_STATE,
  filter: { mine: true, owner: "e_one", flags: ["stale", "overdue"], notRelevant: false, conditional: true },
  text: "  plan  ",
};

describe("the level every view draws from", () => {
  it("is bare at the stock view: every kind at All, nothing else", () => {
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "all", [])).toEqual({ projection: "level", shown: ["group", "decision", "deliverable", "action", "milestone"], display: ["relevant", "conditional"] });
  });

  it("carries the step's kinds, and at Stages only what is under a top-level stage or milestone", () => {
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "decisions", []).shown).not.toContain("deliverable");
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "stages", []).top).toEqual(["group", "milestone"]);
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "work", []).top).toBeUndefined();
  });

  it.each<[string, Partial<ViewState["filter"]>, string[] | undefined]>([
    ["conditional left out", { conditional: false }, ["relevant"]],
    ["not relevant shown", { notRelevant: true }, undefined],
    ["both classes swapped", { notRelevant: true, conditional: false }, ["relevant", "not_relevant"]],
  ])("carries the classes: %s", (_, filter, display) => {
    expect(levelRequestOf({ ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, ...filter } }, "all", []).display).toEqual(display);
  });

  it("carries Only mine, the owner, the flags in the Filter's order and the trimmed search", () => {
    const request = levelRequestOf(narrowed, "all", ["n_stage"]);
    expect([request.flags, request.owner, request.text, request.collapsed]).toEqual([["mine", "overdue", "stale"], "e_one", "plan", ["n_stage"]]);
  });

  it("is the same with Frontier or Decisions on: they only highlight", () => {
    expect(levelRequestOf({ ...narrowed, frontier: true, decisions: true }, "work", [])).toEqual(levelRequestOf(narrowed, "work", []));
  });
});

describe("the list's table and the walk through", () => {
  it("reads the table's rows without the Filter or the search, so a search re-reads only the level", () => {
    expect(tableQueryOf(narrowed, undefined)).toEqual(tableQueryOf(DEFAULT_VIEW_STATE, undefined));
    expect(tableQueryOf(DEFAULT_VIEW_STATE, "due").sort).toBe("due");
    expect(tableQueryOf({ ...DEFAULT_VIEW_STATE, decisions: true }, undefined).kinds).toEqual(["decision"]);
  });

  it("reads the walk's cards from the frontier in rank order, with the Filter and the search", () => {
    expect(walkQueryOf(DEFAULT_VIEW_STATE)).toEqual({ flags: ["next_up"], sort: "rank" });
    expect(walkQueryOf({ ...narrowed, decisions: true })).toEqual({ flags: ["next_up", "mine", "overdue", "stale"], sort: "rank", kinds: ["decision"], owner: "e_one", text: "plan" });
  });
});

describe("the highlights", () => {
  it("light the frontier on the graph, the list and the timeline, and nothing on the walk", () => {
    const frontier = { ...DEFAULT_VIEW_STATE, frontier: true };
    for (const screen of ["graph", "list", "timeline"] as const) {
      expect([...(litOf(screen, frontier, view) ?? [])].sort(), screen).toEqual([...view.derived.acting_frontier].sort());
    }
    expect(litOf("walk", frontier, view)).toBeUndefined();
    expect(litOf("graph", DEFAULT_VIEW_STATE, view)).toBeUndefined();
  });

  it("light the decisions under Decisions on the graph and the timeline; the list reads only decisions instead", () => {
    const decisions = { ...DEFAULT_VIEW_STATE, decisions: true };
    expect(litOf("graph", decisions, view)?.has("n_inner")).toBe(true);
    expect(litOf("timeline", decisions, view)?.has("n_work")).toBe(false);
    expect(litOf("list", decisions, view)).toBeUndefined();
    expect([...(litOf("graph", { ...decisions, frontier: true }, view) ?? [])]).toEqual(["n_pick"]);
  });

  it("light the containers above a lit node", () => {
    expect([...lightsUp(nodes, new Set(["n_inner"]))].sort()).toEqual(["n_inner", "n_stage"]);
  });

  it("never change the layout; the step, the Filter and the search do", () => {
    const stock = journeyLayoutView(DEFAULT_VIEW_STATE, "all");
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, frontier: true, decisions: true }, "all")).toBe(stock);
    expect(journeyLayoutView(DEFAULT_VIEW_STATE, "work")).not.toBe(stock);
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, notRelevant: true } }, "all")).not.toBe(stock);
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, flags: ["stale"] } }, "all")).not.toBe(stock);
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, text: "plan" }, "all")).not.toBe(stock);
  });
});

describe("the badges", () => {
  const byClass = { not_relevant: 3, conditional: 2 };
  const none = () => false;

  it("count what the classes hide on the graph and the list, only conditional on the timeline, nothing on the walk", () => {
    expect(badgesOf("graph", byClass, undefined, none).filter?.count).toBe(5);
    expect(badgesOf("list", byClass, undefined, none).filter?.count).toBe(5);
    expect(badgesOf("timeline", byClass, undefined, none).filter?.count).toBe(2);
    expect(badgesOf("walk", byClass, undefined, none).filter).toBeUndefined();
  });

  it("count on Detail the highlighted nodes the step hides: a kind it does not draw, the Stages rule, or a collapse", () => {
    const base = new Set(view.derived.acting_frontier);
    expect(badgesOf("graph", {}, base, stepHides(nodes, "all", [])).detail).toBeUndefined();
    // Decisions draws no deliverable or action.
    expect(badgesOf("graph", {}, base, stepHides(nodes, "decisions", [])).detail?.count).toBe(2);
    const inside = new Set(["n_inner"]);
    expect(badgesOf("graph", {}, inside, stepHides(nodes, "all", ["n_stage"])).detail?.count).toBe(1);
    // At Stages a top-level decision is left out with what is beneath it.
    expect(stepHides(nodes, "stages", [])("n_pick")).toBe(true);
    expect(stepHides(nodes, "stages", [])("n_inner")).toBe(false);
  });
});
