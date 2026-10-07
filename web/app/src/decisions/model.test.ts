// C12: the decision view as a canvas level (its decisions, their gating edges, their
// hidden-prerequisites markers) and as rows of what each answer affected.
import { describe, expect, it } from "vitest";

import type { Ready } from "../detail/model.ts";
import { testView } from "../detail/view.test-support.ts";
import {
  CANVAS_HEIGHT_MAX_PX,
  CANVAS_HEIGHT_MIN_PX,
  GRID_COLUMN_COUNT_MAX,
  canvasHeightPx,
  decisionLevel,
  decisionPlacement,
  decisionRows,
  type DecisionView,
} from "./model.ts";

/**
 * The test journey plus a gated boolean decision whose "no" makes an optional deliverable and
 * its child not relevant, and a choice decision that waits on the findings.
 */
function decisionJourney(): Ready {
  const view = testView();
  view.journey.graph.nodes = [
    ...(view.journey.graph.nodes ?? []),
    { key: "n_ask", id: "ask", kind: "decision", title: "Ask a partner", prompt: "Does a partner help?", answer_type: "boolean", requires: ["n_when"] },
    { key: "n_optional", id: "optional", kind: "deliverable", title: "Partner work", relevant_when: { equals: { decision: "n_ask", value: true } } },
    { key: "n_optional_step", id: "step", parent: "n_optional", kind: "action", title: "Partner step" },
    { key: "n_lead", id: "lead", kind: "decision", title: "Who leads", answer_type: "entity", fills_role: "r_lead" },
  ];
  view.journey.graph.roles = [{ key: "r_lead", id: "lead", title: "Lead" }];
  const none = { entries: [], total: 0 };
  const node = (value: "relevant" | "not_relevant") => ({ relevance: { value }, actionable: false, gravity: 0, leverage: 0, dates: {}, gravity_from: none, leverage_from: none });
  view.derived.nodes = {
    ...view.derived.nodes,
    n_ask: node("relevant"),
    n_optional: node("not_relevant"),
    n_optional_step: node("not_relevant"),
    n_lead: node("relevant"),
  };
  return view;
}

const projected: DecisionView = {
  decisions: [
    { node: "n_when", state: "decided", relevance: "relevant", answer: { date: "2026-11-20" }, owners: ["e_one"], pins: "n_meeting" },
    { node: "n_ask", state: "decided", relevance: "relevant", answer: { boolean: false }, owners: ["e_one", "e_two"], affects: ["n_optional", "n_optional_step"] },
    { node: "n_lead", state: "open", relevance: "relevant", fills: "r_lead", hidden_prerequisites: ["n_findings"] },
  ],
  edges: [{ from: "n_when", to: "n_ask", gates: true, underlying: [{ requirement: "n_when", dependent: "n_ask", origin: "explicit", gates: true }] }],
};

describe("C12: the decision view", () => {
  it("is the level with only decisions shown: its decisions, gating edges, and markers", () => {
    const level = decisionLevel(projected);
    expect(level.shown).toEqual(["decision"]);
    expect(level.nodes.map((node) => [node.key, node.hidden_prerequisites])).toEqual([
      ["n_when", []],
      ["n_ask", []],
      ["n_lead", ["n_findings"]],
    ]);
    expect(level.edges).toEqual(projected.edges);
  });

  it("shows each answer in effect as people read it, and none for an open decision", () => {
    const rows = decisionRows(decisionJourney(), projected);
    expect(rows.map((row) => [row.key, row.state, row.answer])).toEqual([
      ["n_when", "decided", "2026-11-20"],
      ["n_ask", "decided", "no"],
      ["n_lead", "open", undefined],
    ]);
  });

  it("names what each answer affected: nodes with their relevance now, a pinned milestone with its date, a filled role", () => {
    const [when, ask, lead] = decisionRows(decisionJourney(), projected);
    expect(ask?.affects.map((node) => [node.key, node.title, node.relevance])).toEqual([
      ["n_optional", "Partner work", "not_relevant"],
      ["n_optional_step", "Partner step", "not_relevant"],
    ]);
    expect(when?.pins).toEqual({ key: "n_meeting", title: "Meeting", date: "2026-11-20" });
    expect(lead?.fills).toBe("Lead");
    expect([when?.affects, when?.fills, lead?.pins]).toEqual([[], undefined, undefined]);
  });

  it("names owners, and what a decision waits on that no decision stands for", () => {
    const [when, ask, lead] = decisionRows(decisionJourney(), projected);
    expect([when?.owners, ask?.owners, lead?.owners]).toEqual([["Person One"], ["Person One", "Person Two"], []]);
    expect(lead?.waitsOn).toEqual([{ key: "n_findings", title: "Findings" }]);
  });
});

describe("the decision canvas's layout and height (C12, C15)", () => {
  const keysOf = (count: number) => Array.from({ length: count }, (_, index) => `n_${String(index)}`);
  /** ELK's layout of unconnected cards: one column. */
  const column = (count: number) => Object.fromEntries(keysOf(count).map((key, index) => [key, { x: 0, y: index * 100, width: 240, height: 80 }]));

  it("sets ungated decisions in tree order in a grid, no two in one place", () => {
    for (const count of [1, 4, 7, 30]) {
      const placed = decisionPlacement(keysOf(count), column(count), 0);
      const cells = keysOf(count).map((key) => placed[key]);
      const columns = new Set(cells.map((cell) => cell?.x)).size;
      expect(columns).toBe(Math.min(GRID_COLUMN_COUNT_MAX, Math.ceil(Math.sqrt(count))));
      expect(new Set(cells.map((cell) => `${String(cell?.x)},${String(cell?.y)}`)).size).toBe(count);
      const order = cells.map((cell) => (cell?.y ?? 0) * 10_000 + (cell?.x ?? 0));
      expect([...order].sort((a, b) => a - b)).toEqual(order);
    }
  });

  it("keeps the layered layout when a decision gates another", () => {
    const layered = column(3);
    expect(decisionPlacement(keysOf(3), layered, 1)).toBe(layered);
  });

  it("grows with a column of decisions, within its bounds", () => {
    const heights = [0, 1, 3, 5, 7, 40].map((count) => canvasHeightPx(column(count)));
    expect([...heights].sort((a, b) => a - b)).toEqual(heights);
    expect(heights[0]).toBe(CANVAS_HEIGHT_MIN_PX);
    expect(heights.at(-1)).toBe(CANVAS_HEIGHT_MAX_PX);
    expect(canvasHeightPx(column(3))).toBeGreaterThan(280);
  });
});
