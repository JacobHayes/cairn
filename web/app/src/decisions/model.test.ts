// C12: the decision view as a canvas level (its decisions, their gating edges, their
// hidden-prerequisites markers).
import { describe, expect, it } from "vitest";

import { GRID_COLUMN_COUNT_MAX, decisionLevel, decisionPlacement, type DecisionView } from "./model.ts";

const projected: DecisionView = {
  decisions: [
    { node: "n_when", state: "decided", display_state: "done", relevance: "relevant", answer: { date: "2026-11-20" }, owners: ["e_one"], pins: "n_meeting" },
    { node: "n_ask", state: "decided", display_state: "done", relevance: "relevant", answer: { boolean: false }, owners: ["e_one", "e_two"], affects: ["n_optional", "n_optional_step"] },
    { node: "n_lead", state: "open", display_state: "ready", relevance: "relevant", fills: "r_lead", hidden_prerequisites: ["n_findings"] },
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
});
