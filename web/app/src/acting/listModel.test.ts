// C9, C12: the Plan list's lines: the tree folded to its top level with a container kept for what
// matches beneath it, and the flat order once a column is sorted.
import { describe, expect, it } from "vitest";

import { actingView } from "./acting.test-support.ts";
import { rollups } from "../plan/tree.ts";
import type { DisplayState } from "../status/words.ts";
import { firstParagraph, flatLines, treeLines } from "./listModel.ts";
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

describe("the lines", () => {
  it("fold the tree to its top level, open a container to what it holds, and keep one that matched nothing as context", () => {
    expect(treeLines(view, rows, () => false).map((line) => [line.key, line.depth, line.container, line.open])).toEqual([
      ["n_stage", 0, true, false],
      ["n_work", 0, false, false],
    ]);
    expect(treeLines(view, rows, () => true).map((line) => [line.key, line.depth])).toEqual([["n_stage", 0], ["n_inner", 1], ["n_work", 0]]);
    expect(treeLines(view, [row("n_inner", ["n_stage"])], () => true).map((line) => line.row?.key)).toEqual([undefined, "n_inner"]);
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
