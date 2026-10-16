// C9, C12: the Plan list's lines: the tree of what the level draws, nested by the level and
// folded as asked, and the flat order once a column is sorted.
import { describe, expect, it } from "vitest";

import { actingView } from "./acting.test-support.ts";
import { rollups } from "../plan/tree.ts";
import type { DisplayState } from "../status/words.ts";
import { planOrder } from "../plan/tree.ts";
import type { ListSettings } from "./address.ts";
import { addableColumns, columnsOf, firstParagraph, flatLines, rankPositions, treeLines } from "./listModel.ts";
import type { NodeRow } from "./why.ts";

const view = actingView();

const row = (key: string, ancestors: string[] = []): NodeRow => ({
  key,
  ancestors,
  kind: "action",
  path: key,
  title: key,
  state: "todo",
  display_state: "ready",
  relevance: "relevant",
  gravity: 1,
  unlocks: 0,
});

const rows = [row("n_inner", ["n_stage"]), row("n_work"), row("n_stage")];
const byKey = (each: readonly NodeRow[]) => new Map(each.map((one) => [one.key, one]));
const order = new Map(planOrder(view).map((key, at) => [key, at]));
/** The fixture's level at All: every node drawn under its parent. */
const level = [{ key: "n_stage" }, { key: "n_inner", parent: "n_stage" }, { key: "n_work" }];
const STOCK: ListSettings = { sort: undefined, columns: [] };

describe("rank positions", () => {
  it("follow the engine's own order of the acting frontier when Frontier is on, whatever the keys and the scores say", () => {
    const frontier = actingView();
    frontier.derived.acting_frontier = ["n_work", "n_inner"];
    expect([...rankPositions(frontier, true)]).toEqual([["n_work", 1], ["n_inner", 2]]);
    frontier.derived.acting_frontier = ["n_inner", "n_work"];
    expect([...rankPositions(frontier, true)]).toEqual([["n_inner", 1], ["n_work", 2]]);
  });
});

describe("the columns", () => {
  it("put Rank first only for a sort by rank, once, wherever Columns added it", () => {
    expect(columnsOf(STOCK, false)).toEqual(["status", "title", "owner", "due"]);
    expect(columnsOf({ sort: "rank", columns: ["rank", "unlocks"] }, false)).toEqual(["rank", "status", "title", "owner", "due", "unlocks"]);
    expect(columnsOf({ ...STOCK, columns: ["rank"] }, false)[0]).toBe("status");
  });

  it("show the decision columns after Title at Decisions depth, and no longer offer them in Columns; Kind stays, the rows being mixed", () => {
    expect(columnsOf(STOCK, true)).toEqual(["status", "title", "answer", "why", "affects", "owner", "due"]);
    expect(columnsOf(STOCK, false).some((column) => ["answer", "why", "affects"].includes(column))).toBe(false);
    expect(addableColumns(true)).not.toEqual(expect.arrayContaining(["answer"]));
    expect(addableColumns(true)).toContain("kind");
    expect(addableColumns(false)).toContain("answer");
  });
});

describe("the lines", () => {
  it("nest the drawn nodes as the level draws them, folded or open as asked", () => {
    expect(treeLines(level, byKey(rows), () => false, order).map((line) => [line.key, line.depth, line.container, line.open])).toEqual([
      ["n_stage", 0, true, false],
      ["n_work", 0, false, false],
    ]);
    expect(treeLines(level, byKey(rows), () => true, order).map((line) => [line.key, line.depth])).toEqual([["n_stage", 0], ["n_inner", 1], ["n_work", 0]]);
  });

  it("draw a node under the drawn node the level puts it under, never a hidden ancestor, and skip a drawn node with no row", () => {
    // n_stage is hidden at this level: n_inner is a top-level line, not a child.
    expect(treeLines([{ key: "n_inner" }, { key: "n_work" }], byKey(rows), () => true, order).map((line) => [line.key, line.depth])).toEqual([["n_inner", 0], ["n_work", 0]]);
    // n_stage has no row for a moment: its child stays, at the top.
    expect(treeLines(level, byKey([row("n_inner", ["n_stage"]), row("n_work")]), () => true, order).map((line) => [line.key, line.depth])).toEqual([["n_inner", 0], ["n_work", 0]]);
  });

  it("say per row which guide lines run beside it: its own elbow, and each ancestor's while that ancestor has more below", () => {
    const deep = structuredClone(view);
    const inner = deep.derived.nodes["n_inner"];
    if (inner === undefined) {
      throw new Error("the fixture's inner node has no derivation");
    }
    deep.journey.graph.nodes?.push(
      { key: "n_leaf", id: "leaf", parent: "n_inner", kind: "action", title: "Leaf" },
      { key: "n_zlast", id: "zlast", parent: "n_stage", kind: "action", title: "Last" },
    );
    deep.derived.nodes["n_leaf"] = structuredClone(inner);
    deep.derived.nodes["n_zlast"] = structuredClone(inner);
    const everything = [row("n_inner", ["n_stage"]), row("n_leaf", ["n_stage", "n_inner"]), row("n_zlast", ["n_stage"]), row("n_stage"), row("n_work")];
    const drawn = [{ key: "n_stage" }, { key: "n_inner", parent: "n_stage" }, { key: "n_leaf", parent: "n_inner" }, { key: "n_zlast", parent: "n_stage" }, { key: "n_work" }];
    const placed = new Map(planOrder(deep).map((key, at) => [key, at]));
    expect(treeLines(drawn, byKey(everything), () => true, placed).map((line) => [line.key, line.guides])).toEqual([
      ["n_stage", []],
      ["n_inner", [true]],
      ["n_leaf", [true, false]],
      ["n_zlast", [false]],
      ["n_work", []],
    ]);
  });

  it("keep the engine's order when sorted, put the rows that are over last when sorted by a date, and plan order (the schedule's, undated last) when not", () => {
    const keysOf = (lines: { key: string }[]) => lines.map((line) => line.key);
    expect(keysOf(flatLines(view, rows, "rank"))).toEqual(["n_inner", "n_work", "n_stage"]);
    const over = [{ ...row("n_done"), display_state: "done" as const }, row("n_work")];
    expect(keysOf(flatLines(view, over, "due"))).toEqual(["n_work", "n_done"]);
    expect(keysOf(flatLines(view, [row("n_work"), row("n_later"), row("n_inner", ["n_stage"]), row("n_wait")], undefined))).toEqual(["n_wait", "n_inner", "n_later", "n_work"]);
  });

  it("roll a container up over the leaves that count: a decision still to answer is one to decide, a finished leaf done", () => {
    const showing = (state: DisplayState) => {
      const copy = structuredClone(view);
      const node = copy.derived.nodes["n_inner"];
      if (node !== undefined) {
        node.display_state = state;
      }
      return rollups(copy).get("n_stage");
    };
    expect(showing("ready")).toEqual({ done: 0, total: 1, overdue: 0, toDecide: 1 });
    expect(showing("done")).toEqual({ done: 1, total: 1, overdue: 0, toDecide: 0 });
  });
});

it("a rationale in a cell is its first paragraph as plain text", () => {
  expect(firstParagraph("We chose **Partner**: see [the memo](https://example.org).\n\nMore detail.")).toBe("We chose Partner: see the memo.");
});
