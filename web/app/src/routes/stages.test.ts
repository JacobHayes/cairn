// A route's Stages list: its stages in the order the schedule gives them, with the days each spans.
import { describe, expect, it } from "vitest";

import { treeOf, type Graph } from "../authoring/graph.ts";
import { routeChartOf } from "../timeline/rows.ts";
import { routeStagesOf } from "./stages.ts";

const graph = {
  nodes: [
    { key: "n_late", kind: "group", title: "Late" },
    { key: "n_early", kind: "group", title: "Early" },
    { key: "n_loose", kind: "group", title: "Loose" },
    { key: "n_inner", kind: "group", title: "Inner", parent: "n_early" },
    { key: "n_a", kind: "action", title: "A", parent: "n_early" },
    { key: "n_b", kind: "action", title: "B", parent: "n_late" },
    { key: "n_c", kind: "action", title: "C", parent: "n_late" },
  ],
} as Graph;

const schedule = {
  entries: [
    { node: "n_a", kind: "action" as const, start_day: 0, finish_day: 3 },
    { node: "n_b", kind: "action" as const, start_day: 5, finish_day: 9 },
    { node: "n_c", kind: "action" as const, start_day: 9, finish_day: 9 },
  ],
};

describe("the stages of a route", () => {
  it("lists the top-level groups by the day they start, the stages with none after, each with the days it spans", () => {
    expect(routeStagesOf({ graph }, schedule).map((row) => [row.key, row.due])).toEqual([
      ["n_early", "Start to day 3"],
      ["n_late", "Days 5 to 9"],
      ["n_loose", undefined],
    ]);
  });

  it("puts the stage that finishes sooner first when two start on the same day", () => {
    const tied = { entries: [{ node: "n_a", kind: "action" as const, start_day: 0, finish_day: 8 }, { node: "n_b", kind: "action" as const, start_day: 0, finish_day: 5 }] };
    expect(routeStagesOf({ graph }, tied).slice(0, 2).map((row) => row.key)).toEqual(["n_late", "n_early"]);
  });

  it("orders stages with the same days as the timeline orders them, by the graph's path order", () => {
    const same = { nodes: [{ key: "n_z", kind: "group", title: "Zeta" }, { key: "n_y", kind: "group", title: "Alpha" }, { key: "n_p", kind: "action", title: "P", parent: "n_z" }, { key: "n_q", kind: "action", title: "Q", parent: "n_y" }] } as Graph;
    const days = { entries: [{ node: "n_p", kind: "action" as const, start_day: 1, finish_day: 2 }, { node: "n_q", kind: "action" as const, start_day: 1, finish_day: 2 }] };
    const order = routeStagesOf({ graph: same }, days).map((row) => row.key);
    const chart = routeChartOf(treeOf(same), days, { drawn: same.nodes ?? [], open: () => false, selected: undefined, faded: () => false });
    expect(order).toEqual(chart.rows.map((row) => row.key).filter((key) => order.includes(key)));
  });

  it("keeps the route's own order, with no days, until the schedule arrives, and says nothing of state or progress", () => {
    const rows = routeStagesOf({ graph }, undefined);
    expect(rows.map((row) => [row.key, row.due])).toEqual([["n_late", undefined], ["n_early", undefined], ["n_loose", undefined]]);
    expect(rows.every((row) => row.displayState === undefined && row.done === undefined)).toBe(true);
  });

  it("is empty for a route without stages", () => {
    expect(routeStagesOf({ graph: { nodes: [{ key: "n_a", kind: "action", title: "A" }] } as Graph }, schedule)).toEqual([]);
  });
});
