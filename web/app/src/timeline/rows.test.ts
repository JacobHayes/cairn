// C13, F7, C2: the timeline's rows: the dated nodes the level draws, nested as it draws them and
// folded by the ladder, each row's date in plain words, a container as one bar filled by what is
// done, and what a selection or a highlight leaves faded. Over the detail panel's small journey: a
// stage holding a report and its findings, a final meeting milestone, and a decision with no date.
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { treeOf, type Graph } from "../authoring/graph.ts";
import { dayOf, type Timeline } from "./model.ts";
import { chartOf, routeChartOf, type ChartOptions } from "./rows.ts";

/** The fixture's level at All: every node, each under its parent. */
const whole = [{ key: "n_stage" }, { key: "n_report", parent: "n_stage" }, { key: "n_findings", parent: "n_stage" }, { key: "n_meeting" }, { key: "n_pick" }];
const options: ChartOptions = { drawn: whole, open: () => true, selected: undefined, faded: () => false };

const timeline: Timeline = {
  entries: [
    { node: "n_findings", kind: "deliverable", date: "2026-10-03", origin: "due", overdue: true },
    { node: "n_report", kind: "deliverable", date: "2026-11-02", origin: "pin" },
    { node: "n_meeting", kind: "milestone", date: "2026-11-20", origin: "pin", final: true, shortfall_days: 2 },
  ],
  end: "n_meeting",
};

const keys = (rows: { key: string }[]) => rows.map((row) => row.key);

describe("a snoozed row says where its snooze ends, in the card's words, before its date", () => {
  it("leads a snoozed node's words with the snooze's and leaves another's alone", () => {
    const view = testView();
    const report = view.derived.nodes["n_report"];
    if (report === undefined) {
      throw new Error("the fixture has the report");
    }
    Object.assign(report, { display_state: "snoozed", snoozed: { date: "2026-11-09" } });
    const words = (key: string) => chartOf(view, timeline, options).rows.find((row) => row.key === key)?.words ?? "";
    expect(words("n_report")).toMatch(/^Until Nov 9, /);
    expect(words("n_findings")).not.toMatch(/Until/);
  });
});

describe("the rows are the drawn nodes with dates", () => {
  it("opens a stage to its work as one bar over what is done, puts the final milestone last, and lists the undated", () => {
    const chart = chartOf(testView(), timeline, options);
    expect(chart.rows.map((row) => [row.key, row.depth, row.shape])).toEqual([
      ["n_stage", 0, "span"],
      ["n_report", 1, "tick"],
      ["n_findings", 1, "tick"],
      ["n_meeting", 0, "milestone"],
    ]);
    const stage = chart.rows.find((row) => row.key === "n_stage");
    expect([stage?.from, stage?.to]).toEqual([dayOf("2026-10-03"), dayOf("2026-11-02")]);
    expect(chart.undated).toEqual(["n_pick"]);
    expect(chart.rows.map((row) => [row.key, row.guides])).toEqual([["n_stage", []], ["n_report", [true]], ["n_findings", [false]], ["n_meeting", []]]);
  });

  it("folds a container as asked, which keeps its bar over what it holds", () => {
    const folded = chartOf(testView(), timeline, { ...options, open: (key) => key !== "n_stage" });
    expect(keys(folded.rows)).toEqual(["n_stage", "n_meeting"]);
    expect(folded.rows[0]).toMatchObject({ foldable: true, open: false, from: dayOf("2026-10-03"), to: dayOf("2026-11-02") });
  });

  it("leaves out what the level does not draw: a search's misses, and a hidden container's work goes under nothing", () => {
    const narrowed = chartOf(testView(), timeline, { ...options, drawn: [{ key: "n_stage" }, { key: "n_report", parent: "n_stage" }] });
    expect(keys(narrowed.rows)).toEqual(["n_stage", "n_report"]);
    expect(narrowed.undated).toEqual([]);
    const loose = chartOf(testView(), timeline, { ...options, drawn: [{ key: "n_report" }, { key: "n_meeting" }] });
    expect(loose.rows.map((row) => [row.key, row.depth])).toEqual([["n_report", 0], ["n_meeting", 0]]);
  });

  it("selects the stand-in of a selected node the chart draws no row for", () => {
    const standIn = chartOf(testView(), timeline, { ...options, drawn: [{ key: "n_stage" }, { key: "n_meeting" }], selected: "n_report" });
    expect(standIn.rows.filter((row) => !row.faded).map((row) => row.key)).toContain("n_stage");
  });

  it("lists a selected node that has no row, whatever its state, and no other finished or undrawn one", () => {
    const view = testView();
    const finished = view.derived.nodes["n_pick"];
    if (finished === undefined) {
      throw new Error("no n_pick");
    }
    view.derived.nodes["n_pick"] = { ...finished, display_state: "done" };
    expect(chartOf(view, timeline, options).undated).toEqual([]);
    expect(chartOf(view, timeline, { ...options, selected: "n_pick" }).undated).toEqual(["n_pick"]);
    expect(chartOf(view, timeline, { ...options, drawn: [{ key: "n_stage" }, { key: "n_meeting" }], selected: "n_pick" }).undated).toEqual(["n_pick"]);
  });
});

describe("what each row's mark says", () => {
  const row = (chart: ReturnType<typeof chartOf>, key: string) => chart.rows.find((each) => each.key === key);

  it("draws a task with an earlier start as a bar from there, and one without as a tick on its day", () => {
    const view = testView();
    const report = view.derived.nodes["n_report"];
    if (report === undefined) {
      throw new Error("no report");
    }
    const chain = { constraints: [], fixed: [] };
    expect(row(chartOf(view, timeline, options), "n_report")).toMatchObject({ shape: "tick", from: dayOf("2026-11-02"), to: dayOf("2026-11-02") });
    view.derived.nodes["n_report"] = { ...report, dates: { ...report.dates, latest_start: { date: "2026-10-30", chain } } };
    expect(row(chartOf(view, timeline, options), "n_report")).toMatchObject({ shape: "bar", from: dayOf("2026-10-30"), to: dayOf("2026-11-02") });
  });

  it("marks only the final milestone as the end", () => {
    const chart = chartOf(testView(), timeline, options);
    expect(chart.rows.filter((each) => each.end).map((each) => each.key)).toEqual(["n_meeting"]);
  });
});

describe("a selection and the highlights fade what they do not reach", () => {
  it("draws what the selected row needs and its float, and fades every row the trace does not reach", () => {
    const view = testView();
    const report = view.derived.nodes["n_report"];
    if (report === undefined) {
      throw new Error("no report");
    }
    const chain = { constraints: [], fixed: [] };
    // What it needs is the requirement it names, not only what still holds it.
    view.derived.nodes["n_report"] = { ...report, blocked_by: [], dates: { ...report.dates, earliest_start: { date: "2026-10-20", chain }, latest_start: { date: "2026-10-30", chain } } };
    const chart = chartOf(view, timeline, { ...options, selected: "n_report" });
    expect(chart.dependencies).toEqual([{ from: "n_findings", to: "n_report", kind: "needs" }]);
    expect(chart.rows.filter((row) => !row.faded).map((row) => row.key).sort()).toEqual(["n_findings", "n_report"]);
    expect(chart.rows.filter((row) => row.float !== undefined).map((row) => [row.key, row.float])).toEqual([["n_report", { from: dayOf("2026-10-20"), to: dayOf("2026-10-30") }]]);
    // The axis reaches every row's days and the float's start, which no row stands on.
    expect(chart.days).toEqual(expect.arrayContaining([...chart.rows.flatMap((row) => [row.from, row.to]), dayOf("2026-10-20")]));
    expect(chart.days).not.toEqual([]);
  });

  it("fades the rows a highlight does not light and removes none", () => {
    const lit = chartOf(testView(), timeline, { ...options, faded: (key) => !["n_stage", "n_report"].includes(key) });
    expect(keys(lit.rows)).toEqual(keys(chartOf(testView(), timeline, options).rows));
    expect(lit.rows.filter((row) => row.faded).map((row) => row.key)).toEqual(["n_findings", "n_meeting"]);
  });

  it("keeps an unlit row faded when the trace reaches it: the lowest opacity wins", () => {
    const chart = chartOf(testView(), timeline, { ...options, selected: "n_report", faded: (key) => !["n_stage", "n_report"].includes(key) });
    expect(chart.rows.find((row) => row.key === "n_findings")?.faded).toBe(true);
    expect(chart.rows.find((row) => row.key === "n_report")?.faded).toBe(false);
  });
});

/** A route: a stage holding two works, one waiting on a date decision, a milestone ending it, and a node nothing bounds. */
const route = treeOf({
  nodes: [
    { key: "n_stage", kind: "group", title: "Build" },
    { key: "n_design", kind: "action", title: "Design", parent: "n_stage" },
    { key: "n_build", kind: "action", title: "Build it", parent: "n_stage" },
    { key: "n_gate", kind: "decision", title: "Pick a date" },
    { key: "n_ship", kind: "milestone", title: "Ship" },
    { key: "n_loose", kind: "milestone", title: "Loose end" },
  ],
} as Graph);

const schedule = {
  entries: [
    { node: "n_design", kind: "action" as const, start_day: 0, finish_day: 5 },
    { node: "n_build", kind: "action" as const, start_day: 5, finish_day: 14, waiting: ["n_gate"] },
    { node: "n_ship", kind: "milestone" as const, start_day: 14, finish_day: 14, conditional: true },
  ],
  end: "n_ship",
  undated: [{ node: "n_loose", waiting: ["n_gate"] }],
};

const routeLevel = [{ key: "n_stage" }, { key: "n_design", parent: "n_stage" }, { key: "n_build", parent: "n_stage" }, { key: "n_gate" }, { key: "n_ship" }, { key: "n_loose" }];

describe("C13: a route's rows are the drawn nodes at their earliest days", () => {
  it("nests the work under its stage, whose bar spans it, with the final milestone last", () => {
    const chart = routeChartOf(route, schedule, { ...options, drawn: routeLevel });
    expect(chart.rows.map((row) => [row.key, row.depth, row.shape, row.from, row.to])).toEqual([
      ["n_stage", 0, "span", 0, 14],
      ["n_design", 1, "bar", 0, 5],
      ["n_build", 1, "bar", 5, 14],
      ["n_ship", 0, "milestone", 14, 14],
    ]);
    expect(chart.rows.find((row) => row.key === "n_ship")?.end).toBe(true);
    // The tree's guides: a child's own elbow is its last entry, true while a sibling follows.
    expect(chart.rows.map((row) => [row.key, row.guides])).toEqual([["n_stage", []], ["n_design", [true]], ["n_build", [false]], ["n_ship", []]]);
  });

  it("gathers the drawn nodes with no earliest day in the folded list, and none that is not drawn", () => {
    expect(routeChartOf(route, schedule, { ...options, drawn: routeLevel }).undated).toEqual(["n_loose"]);
    expect(routeChartOf(route, schedule, { ...options, drawn: [{ key: "n_ship" }] }).undated).toEqual([]);
  });

  it("says which unanswered date a node with no day waits on", () => {
    const chart = routeChartOf(route, schedule, { ...options, drawn: routeLevel });
    expect(chart.undatedWords?.get("n_loose")).toContain("Pick a date");
    expect(chart.undatedWords?.has("n_design")).toBe(false);
  });

  it("spans a deliverable's own work after its children's, and a stage by its children alone", () => {
    const nested = treeOf({
      nodes: [
        { key: "n_stage", kind: "group", title: "Stage" },
        { key: "n_deliverable", kind: "deliverable", title: "Deliverable", parent: "n_stage" },
        { key: "n_step", kind: "action", title: "Step", parent: "n_deliverable" },
      ],
    } as Graph);
    const days = {
      entries: [
        { node: "n_stage", kind: "group" as const, start_day: 0, finish_day: 20 },
        { node: "n_deliverable", kind: "deliverable" as const, start_day: 3, finish_day: 8 },
        { node: "n_step", kind: "action" as const, start_day: 0, finish_day: 3 },
      ],
    };
    const drawn = [{ key: "n_stage" }, { key: "n_deliverable", parent: "n_stage" }, { key: "n_step", parent: "n_deliverable" }];
    const chart = routeChartOf(nested, days, { ...options, drawn, open: () => true });
    const span = (key: string) => chart.rows.find((row) => row.key === key);
    expect([span("n_deliverable")?.from, span("n_deliverable")?.to]).toEqual([0, 8]);
    // The stage's own bounds do not stretch it past what it holds.
    expect([span("n_stage")?.from, span("n_stage")?.to]).toEqual([0, 8]);
  });

  it("marks a node in an undecided branch conditional and says what a row's date waits on", () => {
    const chart = routeChartOf(route, schedule, { ...options, drawn: routeLevel });
    expect(chart.rows.find((row) => row.key === "n_ship")?.state).toBe("conditional");
    expect(chart.rows.find((row) => row.key === "n_design")?.state).not.toBe("conditional");
    expect(chart.rows.find((row) => row.key === "n_build")?.words).toContain("Pick a date");
    expect(chart.rows.find((row) => row.key === "n_design")?.words).not.toContain("waits");
  });

  it("draws only what the level draws, and folds a stage as asked", () => {
    const narrowed = routeChartOf(route, schedule, { ...options, drawn: [{ key: "n_design" }, { key: "n_ship" }] });
    expect(keys(narrowed.rows)).toEqual(["n_design", "n_ship"]);
    const folded = routeChartOf(route, schedule, { ...options, drawn: routeLevel, open: (key) => key !== "n_stage" });
    expect(keys(folded.rows)).toEqual(["n_stage", "n_ship"]);
    expect(folded.rows[0]).toMatchObject({ foldable: true, open: false, from: 0, to: 14 });
  });

  it("orders siblings by their earliest day, not the graph's order", () => {
    const reversed = { ...schedule, entries: schedule.entries.map((entry) => (entry.node === "n_design" ? { ...entry, start_day: 6, finish_day: 9 } : entry)) };
    const chart = routeChartOf(route, reversed, { ...options, drawn: routeLevel });
    expect(chart.rows.filter((row) => row.depth === 1).map((row) => row.key)).toEqual(["n_build", "n_design"]);
  });
});
