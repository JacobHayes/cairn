// C2, C9, C10, C11, C13: each shared setting reaches every view's requests. Depth, the Filter
// and the search are in the one level request every view draws from; the list's table and the
// walk through read their rows beside it; Frontier lights nodes and Fade dims those below the
// depth, and neither changes what is drawn or how it is laid out.
import { describe, expect, it } from "vitest";

import { actingView } from "../acting/acting.test-support.ts";
import { canvasView } from "../canvas/canvas.test-support.ts";
import { collapsedAt, drawnStep, kindsAt } from "../canvas/ladder.ts";
import { STEPS, type Level, type Step } from "../canvas/model.ts";
import type { GraphNode } from "../detail/model.ts";
import { badgesOf, currentStagesOf, depthFaded, journeyLayoutView, levelRequestOf, lightsUp, litOf, outOfFocus, selectedRequest, stepHides, tableQueryOf, walkQueryOf } from "./requests.ts";
import { DEFAULT_FILTER, DEFAULT_VIEW_STATE, type ViewState } from "./view.ts";

const view = actingView();
const nodes = view.journey.graph.nodes ?? [];

const narrowed: ViewState = {
  ...DEFAULT_VIEW_STATE,
  filter: { mine: true, owner: "who_one", flags: ["stale", "overdue"], notRelevant: false, conditional: true },
  text: "  plan  ",
};

describe("the level every view draws from", () => {
  it("is bare at the stock view: every kind at Actions, nothing else", () => {
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "actions", [])).toEqual({ projection: "level", shown: ["group", "decision", "deliverable", "action", "milestone"], display: ["relevant", "conditional"] });
  });

  it("carries the step's kinds, and at Stages only what is under a top-level stage or milestone", () => {
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "decisions", []).shown).not.toContain("deliverable");
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "stages", []).top).toEqual(["group", "milestone"]);
    expect(levelRequestOf(DEFAULT_VIEW_STATE, "deliverables", []).top).toBeUndefined();
  });

  it.each<[string, Partial<ViewState["filter"]>, string[] | undefined]>([
    ["conditional left out", { conditional: false }, ["relevant"]],
    ["not relevant shown", { notRelevant: true }, undefined],
    ["both classes swapped", { notRelevant: true, conditional: false }, ["relevant", "not_relevant"]],
  ])("carries the classes: %s", (_, filter, display) => {
    expect(levelRequestOf({ ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, ...filter } }, "actions", []).display).toEqual(display);
  });

  it("carries Only mine, the owner, the flags in the Filter's order and the trimmed search", () => {
    const request = levelRequestOf(narrowed, "actions", ["n_stage"]);
    expect([request.flags, request.owner, request.text, request.collapsed]).toEqual([["mine", "overdue", "stale"], "who_one", "plan", ["n_stage"]]);
  });

  it("is the same with Frontier on: it only lights", () => {
    expect(levelRequestOf({ ...narrowed, frontier: true }, "deliverables", [])).toEqual(levelRequestOf(narrowed, "deliverables", []));
  });
});

describe("the level that draws the selected node (C7)", () => {
  const request = levelRequestOf(DEFAULT_VIEW_STATE, "actions", []);
  /** The part of a level the decision reads: what it draws and what it shows collapsed. */
  const answer = (drawn: string[], collapsed: string[] = []) => ({ nodes: drawn.map((key) => ({ key })), collapsed }) as unknown as Pick<Level, "nodes" | "collapsed">;

  it("is none while the level draws the node and every container above it open", () => {
    expect(selectedRequest(request, answer(["n_stage", "n_inner"]), "n_inner", nodes)).toBeUndefined();
  });

  it("names the node when the level does not draw it, or draws it under a container it shows collapsed or does not draw", () => {
    const named = { ...request, selected: "n_inner" };
    expect(selectedRequest(request, answer(["n_stage"]), "n_inner", nodes)).toEqual(named);
    expect(selectedRequest(request, answer(["n_stage", "n_inner"], ["n_stage"]), "n_inner", nodes)).toEqual(named);
    expect(selectedRequest(request, answer(["n_inner"]), "n_inner", nodes)).toEqual(named);
  });

  it("is none for no selection, a node the journey lacks, or a level not read yet; a collapsed selection is the container itself", () => {
    expect(selectedRequest(request, answer([]), undefined, nodes)).toBeUndefined();
    expect(selectedRequest(request, answer([]), "n_nowhere", nodes)).toBeUndefined();
    expect(selectedRequest(request, undefined, "n_inner", nodes)).toBeUndefined();
    expect(selectedRequest(request, answer(["n_stage"], ["n_stage"]), "n_stage", nodes)).toBeUndefined();
  });

  it("stops at the container drilled into", () => {
    const drilled = { ...request, container: "n_stage" };
    expect(selectedRequest(drilled, answer(["n_inner"]), "n_inner", nodes)).toBeUndefined();
  });
});

describe("the list's table and the walk through", () => {
  it("reads the table's rows whatever the depth, the Filter or the search, so a change re-reads only the level", () => {
    expect(tableQueryOf(undefined)).toEqual({ sort: "rank" });
    expect(tableQueryOf("due").sort).toBe("due");
  });

  it("reads the walk's cards from the frontier in rank order, with the Filter and the search, and only decisions at Decisions depth", () => {
    expect(walkQueryOf(DEFAULT_VIEW_STATE, "actions")).toEqual({ flags: ["next_up"], sort: "rank" });
    expect(walkQueryOf(narrowed, "decisions")).toEqual({ flags: ["next_up", "mine", "overdue", "stale"], sort: "rank", kinds: ["decision"], owner: "who_one", text: "plan" });
    for (const depth of STEPS.filter((each) => each !== "decisions")) {
      expect(walkQueryOf(DEFAULT_VIEW_STATE, depth).kinds, depth).toBeUndefined();
    }
  });
});

describe("every depth", () => {
  /** The kind each step is named after, the deepest it draws. */
  const named: Record<Step, string | undefined> = { stages: undefined, decisions: "decision", deliverables: "deliverable", actions: "action" };

  it.each(STEPS)("%s: the level asks for the step's kinds under Hide and for every kind under Fade", (depth) => {
    const hide = levelRequestOf(DEFAULT_VIEW_STATE, drawnStep(depth, false), []);
    const fade = levelRequestOf({ ...DEFAULT_VIEW_STATE, fade: true }, drawnStep(depth, true), []);
    expect(hide.shown).toEqual(kindsAt(depth));
    expect(fade.shown).toEqual(kindsAt("actions"));
    // Stages' rule (only under a top-level stage or milestone) is Hide's alone.
    expect([hide.top !== undefined, fade.top !== undefined]).toEqual([depth === "stages", false]);
  });

  it("draws down to the kind it is named after, and nothing deeper", () => {
    const deeper: Record<string, string[]> = { decision: ["deliverable", "action"], deliverable: ["action"], action: [] };
    for (const depth of STEPS) {
      const kind = named[depth];
      if (kind !== undefined) {
        expect(kindsAt(depth), depth).toContain(kind);
        expect(kindsAt(depth).filter((each) => deeper[kind]?.includes(each)), depth).toEqual([]);
      }
    }
  });

  it("under Fade, the Stages collapse is the viewer's alone: the non-current stages are drawn", () => {
    const stages = [node("s1", "group"), node("s2", "group")];
    const current = new Set(["s1"]);
    expect(collapsedAt(drawnStep("stages", false), stages, current, [], [])).toEqual(["s2"]);
    expect(collapsedAt(drawnStep("stages", true), stages, current, [], ["s1"])).toEqual(["s1"]);
  });
});

const node = (key: string, kind: GraphNode["kind"], parent?: string): GraphNode => ({ key, id: key, kind, title: key, ...(parent === undefined ? {} : { parent }) });

describe("what Fade dims", () => {
  // A kickoff milestone, a top-level decision and deliverable, a stage holding a deliverable that
  // holds an action and a decision, and a top-level action.
  const journey = [
    node("m", "milestone"),
    node("d", "decision"),
    node("o", "deliverable"),
    node("s", "group"),
    node("sb", "deliverable", "s"),
    node("sa", "action", "sb"),
    node("sd", "decision", "sb"),
    node("t", "action"),
  ];
  const keys = new Set(journey.map((each) => each.key));
  const fade = { ...DEFAULT_VIEW_STATE, fade: true };
  const dimmed = (depth: Step, current: string[] = []) => [...depthFaded(fade, journey, depth, collapsedAt(depth, journey, new Set(current), [], []), keys)].sort();

  it("is nothing with Fade off", () => {
    expect(depthFaded(DEFAULT_VIEW_STATE, journey, "decisions", [], keys).size).toBe(0);
  });

  it("is nothing at Actions: it draws every kind", () => {
    expect(dimmed("actions")).toEqual([]);
  });

  it("at Decisions dims the deliverables and actions, but not the deliverable holding a decision", () => {
    expect(dimmed("decisions")).toEqual(["o", "sa", "t"]);
  });

  it("at Deliverables dims the actions", () => {
    expect(dimmed("deliverables")).toEqual(["sa", "t"]);
  });

  it("at Stages dims what Hide leaves out: a top-level non-stage, an action, and the contents of a stage that is not current", () => {
    expect(dimmed("stages", ["s"])).toEqual(["d", "o", "sa", "t"]);
    expect(dimmed("stages")).toEqual(["d", "o", "sa", "sb", "sd", "t"]);
  });

  it("clears no container above a node the level does not draw", () => {
    // The decision under the deliverable is not drawn (a filter hides it), so nothing holds a node in focus there.
    const level = new Set(["m", "d", "o", "s", "sb", "sa", "t"]);
    expect([...depthFaded(fade, journey, "decisions", [], level)].sort()).toEqual(["o", "sa", "sb", "t"]);
  });
});

describe("what is out of focus", () => {
  const lit = new Set(["a", "b"]);
  const faded = new Set(["b", "c"]);

  it("is a node outside what Frontier lights, or inside what Depth dims, or both, once", () => {
    const out = outOfFocus(lit, faded);
    // a: lit and drawn at the depth; b: lit but below the depth; c: not lit and below the depth; d: not lit.
    expect(["a", "b", "c", "d"].map(out)).toEqual([false, true, true, true]);
  });

  it("is decided by Depth alone while Frontier is off", () => {
    const out = outOfFocus(undefined, faded);
    expect(["a", "b", "c"].map(out)).toEqual([false, true, true]);
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

  it("light the containers above a lit node", () => {
    expect([...lightsUp(nodes, new Set(["n_inner"]))].sort()).toEqual(["n_inner", "n_stage"]);
  });

  it("never change the layout; the step it draws, the Filter and the search do", () => {
    const stock = journeyLayoutView(DEFAULT_VIEW_STATE, "actions");
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, frontier: true }, "actions")).toBe(stock);
    expect(journeyLayoutView(DEFAULT_VIEW_STATE, "deliverables")).not.toBe(stock);
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, notRelevant: true } }, "actions")).not.toBe(stock);
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, flags: ["stale"] } }, "actions")).not.toBe(stock);
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, text: "plan" }, "actions")).not.toBe(stock);
  });

  it.each(STEPS)("Fade leaves the layout where Actions has it, whichever depth is chosen: %s", (depth) => {
    const stock = journeyLayoutView(DEFAULT_VIEW_STATE, "actions");
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, fade: true }, depth)).toBe(stock);
    expect(journeyLayoutView({ ...DEFAULT_VIEW_STATE, fade: true, frontier: true }, depth)).toBe(stock);
    // Hide at a shallower depth lays out its own, smaller level.
    expect(journeyLayoutView(DEFAULT_VIEW_STATE, depth) === stock).toBe(depth === "actions");
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

  it("count on Depth the frontier nodes the step hides: a kind it does not draw, the Stages rule, or a collapse; none under Fade", () => {
    const base = new Set(view.derived.acting_frontier);
    expect(badgesOf("graph", {}, base, stepHides(nodes, "actions", [])).depth).toBeUndefined();
    // Decisions draws no deliverable or action.
    expect(badgesOf("graph", {}, base, stepHides(nodes, "decisions", [])).depth?.count).toBe(2);
    expect(badgesOf("graph", {}, base, stepHides(nodes, "decisions", []), true).depth).toBeUndefined();
    const inside = new Set(["n_inner"]);
    expect(badgesOf("graph", {}, inside, stepHides(nodes, "actions", ["n_stage"])).depth?.count).toBe(1);
    // At Stages a top-level decision is left out with what is beneath it.
    expect(stepHides(nodes, "stages", [])("n_pick")).toBe(true);
    expect(stepHides(nodes, "stages", [])("n_inner")).toBe(false);
  });
});

describe("the current stages (C2)", () => {
  it("count the work the engine shows active, so work skipped through its container does not make a stage current", () => {
    const ready = canvasView();
    ready.derived.acting_frontier = [];
    expect(currentStagesOf(ready).has("n_stage")).toBe(true);
    // The engine shows a skipped container's whole subtree skipped, whatever each node stored.
    for (const key of ["n_stage", "n_build", "n_check"]) {
      const found = ready.derived.nodes[key];
      if (found !== undefined) {
        ready.derived.nodes[key] = { ...found, display_state: "skipped" };
      }
    }
    expect(currentStagesOf(ready).has("n_stage")).toBe(false);
  });
});
