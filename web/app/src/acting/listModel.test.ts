// C9, C12: the Plan list's lines: the tree of what the level draws, nested by the level and
// folded as asked, and the flat order once a column is sorted.
import { describe, expect, it } from "vitest";

import { actingView } from "./acting.test-support.ts";
import { containerKeys } from "../plan/tree.ts";
import { planOrder } from "../plan/tree.ts";
import type { ListSettings } from "./address.ts";
import { addableColumns, columnsOf, firstParagraph, flatLines, treeLines } from "./listModel.ts";
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
const containers = containerKeys(view.journey.graph.nodes ?? []);
/** The fixture's level at All: every node drawn under its parent. */
const level = [{ key: "n_stage" }, { key: "n_inner", parent: "n_stage" }, { key: "n_work" }];
const STOCK: ListSettings = { sort: undefined, columns: [] };

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
    expect(treeLines(level, byKey(rows), () => false, order, containers).map((line) => [line.key, line.depth, line.container, line.open])).toEqual([
      ["n_stage", 0, true, false],
      ["n_work", 0, false, false],
    ]);
    expect(treeLines(level, byKey(rows), () => true, order, containers).map((line) => [line.key, line.depth])).toEqual([["n_stage", 0], ["n_inner", 1], ["n_work", 0]]);
  });

  it("keep a container a container when the level draws none of what it holds, with nothing to fold", () => {
    const [stage] = treeLines([{ key: "n_stage" }, { key: "n_work" }], byKey(rows), () => true, order, containers);
    expect(stage).toMatchObject({ key: "n_stage", container: true, foldable: false });
  });

  it("draw a node under the drawn node the level puts it under, never a hidden ancestor, and skip a drawn node with no row", () => {
    // n_stage is hidden at this level: n_inner is a top-level line, not a child.
    expect(treeLines([{ key: "n_inner" }, { key: "n_work" }], byKey(rows), () => true, order, containers).map((line) => [line.key, line.depth])).toEqual([["n_inner", 0], ["n_work", 0]]);
    // n_stage has no row for a moment: its child stays, at the top.
    expect(treeLines(level, byKey([row("n_inner", ["n_stage"]), row("n_work")]), () => true, order, containers).map((line) => [line.key, line.depth])).toEqual([["n_inner", 0], ["n_work", 0]]);
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
    expect(treeLines(drawn, byKey(everything), () => true, placed, containers).map((line) => [line.key, line.guides])).toEqual([
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
    expect(keysOf(flatLines(view, [row("n_work"), row("n_later"), row("n_inner", ["n_stage"]), row("n_wait")], undefined))).toEqual(["n_inner", "n_later", "n_wait", "n_work"]);
  });
});

describe("snoozed nodes", () => {
  it("order a snoozed node by when its snooze lifts: later of its own date and the lift, undated when the node it waits on has no finish date", () => {
    const snoozedWith = (snoozed: { date: string } | { node: string }, finish?: string) => {
      const copy = structuredClone(view);
      const wait = copy.derived.nodes["n_wait"];
      const work = copy.derived.nodes["n_work"];
      if (wait === undefined || work === undefined) {
        throw new Error("the fixture has both");
      }
      wait.snoozed = snoozed;
      work.dates = finish === undefined ? {} : { due: { date: finish, chain: { constraints: [], fixed: [] } } };
      return planOrder(copy).filter((key) => ["n_wait", "n_inner", "n_later", "n_work"].includes(key));
    };
    // Its own date is Oct 6, the soonest of all; a snooze to Oct 30 puts it after the node dated Oct 20.
    expect(snoozedWith({ date: "2026-10-30" })).toEqual(["n_inner", "n_later", "n_wait", "n_work"]);
    // A snooze lifting before its own date moves nothing.
    expect(snoozedWith({ date: "2026-10-01" })).toEqual(["n_wait", "n_inner", "n_later", "n_work"]);
    // Until a node is done: at that node's finish date, or undated, after the dated, when it has none.
    expect(snoozedWith({ node: "n_work" }, "2026-10-16")).toEqual(["n_inner", "n_wait", "n_work", "n_later"]);
    expect(snoozedWith({ node: "n_work" })).toEqual(["n_inner", "n_later", "n_wait", "n_work"]);
  });

  it("wait for the latest hold: a node snoozed itself and under a snoozed container comes back with the container", () => {
    const copy = structuredClone(view);
    const [inner, stage, later] = [copy.derived.nodes["n_inner"], copy.derived.nodes["n_stage"], copy.derived.nodes["n_later"]];
    if (inner === undefined || stage === undefined || later === undefined) {
      throw new Error("the fixture has all three");
    }
    Object.assign(inner, { display_state: "snoozed", snoozed: { date: "2026-10-25" } });
    Object.assign(stage, { display_state: "snoozed", snoozed: { date: "2026-10-30" } });
    later.dates = { earliest_start: { date: "2026-10-27", chain: { constraints: [], fixed: [] } } };
    const order = planOrder(copy).filter((key) => ["n_stage", "n_later"].includes(key));
    expect(order).toEqual(["n_later", "n_stage"]);
  });

  it("put snoozed rows after the others when sorted by a signal, keeping the order among them", () => {
    const snoozed = (key: string) => ({ ...row(key), display_state: "snoozed" as const });
    const keysOf = (lines: { key: string }[]) => lines.map((line) => line.key);
    const mixed = [snoozed("n_pick"), row("n_work"), snoozed("n_meet"), row("n_later")];
    for (const sort of ["rank", "slack", "gravity", "unlocks", "due"] as const) {
      expect(keysOf(flatLines(view, mixed, sort))).toEqual(["n_work", "n_later", "n_pick", "n_meet"]);
    }
  });
});

it("a rationale in a cell is its first paragraph as plain text", () => {
  expect(firstParagraph("We chose **Partner**: see [the memo](https://example.org).\n\nMore detail.")).toBe("We chose Partner: see the memo.");
});
