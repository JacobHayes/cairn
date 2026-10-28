import { describe, expect, it } from "vitest";

import { onlyTraced, withSelectable } from "./graph.ts";
import { endsOf, lineOf, type Card, type CanvasModel, type LineLink } from "./model.ts";

const card = (key: string, parent?: string): Card => ({ key, kind: "action", title: key, parent, stage: false, collapsed: false, rolledUp: [], hiddenPrerequisites: [], matched: false, journey: undefined });
const edge = (requirement: string, dependent: string, extra: Partial<LineLink> = {}): LineLink => ({ requirement, dependent, kind: "requires", holds: false, sentence: "", ...extra });

describe("onlyTraced: lines that land on one pair of cards", () => {
  // Both lines run to cards inside "n_hidden", which Reveal leaves out, so each is drawn to it.
  const model = (first: LineLink, second: LineLink): CanvasModel => ({
    cards: [card("n_from"), card("n_hidden"), card("n_inner", "n_hidden"), card("n_other", "n_hidden")],
    lines: [lineOf("n_from->n_inner", "n_from", "n_inner", [first]), lineOf("n_from->n_other", "n_from", "n_other", [second])],
  });
  const merged = (first: LineLink, second: LineLink) => onlyTraced(model(first, second), new Set(), new Set(["n_from", "n_hidden"])).lines;

  it("is one line that keeps both lines' edges, with the stronger kind", () => {
    const [line, ...rest] = merged(edge("n_from", "n_inner", { kind: "condition" }), edge("n_from", "n_other"));
    expect(rest).toEqual([]);
    expect([line?.id, line?.kind, line?.edges.map((each) => each.dependent)]).toEqual(["n_from->n_hidden", "requires", ["n_other", "n_inner"]]);
  });

  it("stays without state when neither edge has any, and is not satisfied when one still waits", () => {
    expect(merged(edge("n_from", "n_inner"), edge("n_from", "n_other"))[0]?.satisfied).toBeUndefined();
    expect(merged(edge("n_from", "n_inner", { waits: false }), edge("n_from", "n_other", { waits: true }))[0]?.satisfied).toBe(false);
    expect(merged(edge("n_from", "n_inner", { waits: false }), edge("n_from", "n_other", { waits: false }))[0]?.satisfied).toBe(true);
  });
});

describe("a line opens its card only where the view's own level draws it", () => {
  const model: CanvasModel = { cards: [], lines: [lineOf("n_a->n_b", "n_a", "n_b", [edge("n_a", "n_b")]), lineOf("n_b->n_c", "n_b", "n_c", [edge("n_b", "n_c")])] };

  it("marks the lines the own level lacks, and every line when there is no restriction", () => {
    expect(withSelectable(model, new Set(["n_a->n_b"])).lines.map((line) => line.selectable)).toEqual([true, false]);
    expect(withSelectable(model, new Set()).lines.map((line) => line.selectable)).toEqual([false, false]);
    expect(withSelectable(model, undefined).lines.map((line) => line.selectable)).toEqual([true, true]);
  });
});

describe("the cards an address names", () => {
  it("are exactly two keys", () => {
    expect(endsOf("a~b")).toEqual({ from: "a", to: "b" });
    for (const bad of ["a", "a~", "~b", "a~b~c", ""]) {
      expect(endsOf(bad)).toBeUndefined();
    }
  });
});
