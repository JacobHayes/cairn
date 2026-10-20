// C14: the proposal layer's pure parts: which nodes are marked how, how removed things are put
// back in place on a canvas and in a list, the entities' diff, which revision Apply sends, and
// the one gate every live write reads.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { Graph } from "../authoring/graph.ts";
import type { Card, Line } from "../canvas/model.ts";
import type { Deployment } from "../data/host.ts";
import { applyRevision, entityDiff, LayerContext, layerMarks, useReviewing, withRemoved, withRemovedRows, type Layer } from "./layer.ts";

const node = (key: string, parent?: string, requires: string[] = []) => ({ key, id: key, kind: "action" as const, title: key, ...(parent === undefined ? {} : { parent }), ...(requires.length === 0 ? {} : { requires }) });

const before: Graph = { nodes: [node("n_a"), node("n_b", "n_a"), node("n_c", undefined, ["n_b"])], retired_keys: {} };
const after: Graph = { nodes: [node("n_a"), node("n_c"), node("n_d", "n_a", ["n_a"])], retired_keys: { nodes: ["n_b"] } };

describe("layerMarks (C14)", () => {
  it("marks added, changed and removed nodes, and says which insertion an added one belongs to", () => {
    const { marks, entries } = layerMarks({ before, after, items: [], unresolved: new Set(), known: true, members: { n_d: "Security review" } });
    expect(Object.fromEntries(Object.entries(marks).map(([key, mark]) => [key, [mark.kind, mark.note]]))).toEqual({
      n_b: ["removed", undefined],
      n_c: ["changed", undefined],
      n_d: ["added", "Security review"],
    });
    expect(entries.map((entry) => entry.key)).toEqual(["n_d", "n_c", "n_b"]);
  });

  it("marks nothing while the destination after is not known", () => {
    expect(layerMarks({ before, after: undefined, items: [], unresolved: new Set(), known: false, members: {} }).marks).toEqual({});
  });

  it("asks for a choice only on the node of a conflict still unresolved", () => {
    const items = [{ item: "conflict" as const, conflict: { about: "field" as const, node: "n_c", journey: { title: "C" }, route: { title: "See" } } }];
    const asked = layerMarks({ before, after, items, unresolved: new Set([0]), known: true, members: {} }).marks;
    const settled = layerMarks({ before, after, items, unresolved: new Set(), known: true, members: {} }).marks;
    expect([asked["n_c"]?.choice, settled["n_c"]?.choice]).toEqual([true, undefined]);
  });
});

const card = (key: string, parent?: string, rolledUp: string[] = []): Card => ({ key, kind: "action", title: key, parent, stage: false, collapsed: rolledUp.length > 0, checklist: [], rolledUp, hiddenPrerequisites: [], journey: undefined });
const line = (from: string, to: string): Line => ({ id: `${from}->${to}`, from, to, implicit: false, gates: true, kind: "requires", count: 1, sentence: "" });

describe("withRemoved (C14)", () => {
  const was = { cards: [card("n_a"), card("n_b", "n_a"), card("n_c")], lines: [line("n_b", "n_c")] };
  const now = { cards: [card("n_a"), card("n_c")], lines: [] };

  it("puts a removed card back under its parent with the lines that touch it", () => {
    const composed = withRemoved(now, was, new Set(["n_b"]), []);
    expect(composed.model.cards.map((each) => [each.key, each.parent])).toEqual([["n_a", undefined], ["n_c", undefined], ["n_b", "n_a"]]);
    expect([...composed.removedLines]).toEqual(["n_b->n_c"]);
  });

  it("draws a removed requirement between two kept cards as a removed line", () => {
    const composed = withRemoved({ cards: [card("n_a"), card("n_c")], lines: [] }, { cards: [card("n_a"), card("n_c")], lines: [] }, new Set(), [{ node: "n_c", requires: "n_a" }]);
    expect(composed.model.lines.map((each) => each.id)).toEqual(["n_a->n_c"]);
    expect([...composed.removedLines]).toEqual(["n_a->n_c"]);
  });

  it("marks a kept card that lost nodes rolled up into it instead of drawing them", () => {
    const rolled = { cards: [card("n_a", undefined, ["n_b"])], lines: [] };
    const composed = withRemoved({ cards: [card("n_a", undefined, [])], lines: [] }, rolled, new Set(["n_b"]), []);
    expect(composed.model.cards.map((each) => each.key)).toEqual(["n_a"]);
    expect(composed.shrunk.get("n_a")).toEqual(["n_b"]);
  });

  it("changes nothing when nothing is removed", () => {
    expect(withRemoved(now, was, new Set(), []).model).toBe(now);
  });
});

describe("withRemovedRows (C14)", () => {
  const row = (key: string) => ({ key });
  const entry = (key: string, parent?: string) => ({ key, ...(parent === undefined ? {} : { parent }) });

  it("puts a removed row back after the row that came before it, under its own parent", () => {
    const merged = withRemovedRows({
      entries: [entry("n_a"), entry("n_c")],
      rows: [row("n_a"), row("n_c")],
      was: { entries: [entry("n_a"), entry("n_b", "n_a"), entry("n_c")], rows: [row("n_a"), row("n_b"), row("n_c")] },
      removed: new Set(["n_b"]),
      order: ["n_a", "n_c"],
      wasOrder: ["n_a", "n_b", "n_c"],
    });
    expect(merged.order).toEqual(["n_a", "n_b", "n_c"]);
    expect(merged.entries.find((each) => each.key === "n_b")?.parent).toBe("n_a");
    expect([...merged.fromBefore]).toEqual(["n_b"]);
    expect(merged.rows.map((each) => each.key)).toEqual(["n_a", "n_c", "n_b"]);
  });

  it("leaves a row the level after still draws alone, even if it is marked removed", () => {
    const merged = withRemovedRows({ entries: [entry("n_a")], rows: [row("n_a")], was: { entries: [entry("n_a")], rows: [row("n_a")] }, removed: new Set(["n_a"]), order: ["n_a"], wasOrder: ["n_a"] });
    expect([merged.rows.length, merged.fromBefore.size]).toEqual([1, 0]);
  });
});

describe("entityDiff (C14)", () => {
  const entity = (key: string, name: string, emails: string[] = []) => ({ key, name, emails });
  const deployment = (entities: ReturnType<typeof entity>[], aliases: Record<string, string> = {}) => ({ revision: 1, entities, aliases }) as unknown as Deployment;

  it("marks a new entity added, an edited one changed, and a merged one removed saying whom it went into", () => {
    const rows = entityDiff(
      deployment([entity("e_a", "Ana"), entity("e_b", "Bo"), entity("e_c", "Cy")]),
      deployment([entity("e_a", "Ana R."), entity("e_c", "Cy", ["cy@example.test"]), entity("e_d", "Di")], { e_b: "e_c" }),
    );
    expect(rows.map((each) => [each.entity.key, each.mark?.kind, each.mark?.into])).toEqual([
      ["e_a", "changed", undefined],
      ["e_b", "removed", "e_c"],
      ["e_c", "changed", undefined],
      ["e_d", "added", undefined],
    ]);
  });

  it("lists the rows by name, a created entity among them rather than last", () => {
    const rows = entityDiff(deployment([entity("e_b", "Bea"), entity("e_d", "Dev")]), deployment([entity("e_b", "Bea"), entity("e_d", "Dev"), entity("e_a", "Ada")]));
    expect(rows.map((each) => [each.entity.key, each.mark?.kind])).toEqual([["e_a", "added"], ["e_b", undefined], ["e_d", undefined]]);
  });

  it("marks nothing while there is no deployment after", () => {
    expect(entityDiff(deployment([entity("e_a", "Ana")]), undefined).map((each) => each.mark)).toEqual([undefined]);
  });
});

describe("applyRevision (I6)", () => {
  it("sends the revision on screen only when it is the latest the page has read", () => {
    expect([applyRevision(4, 4), applyRevision(5, 4), applyRevision(4, undefined)]).toEqual([4, undefined, undefined]);
  });
});

describe("useReviewing (C14)", () => {
  const Probe = () => <span>{useReviewing() === undefined ? "free" : "held"}</span>;
  const layer = (mode: Layer["mode"]) => ({ mode }) as Layer;
  const seen = (value: Layer | undefined) => renderToStaticMarkup(<LayerContext value={value}><Probe /></LayerContext>);

  it("holds live writes only while a proposal is under review", () => {
    expect([seen(undefined), seen(layer("reviewing")), seen(layer("applied")), seen(layer("discarded")), seen(layer("missing"))]).toEqual([
      "<span>free</span>",
      "<span>held</span>",
      "<span>free</span>",
      "<span>free</span>",
      "<span>free</span>",
    ]);
  });
});
