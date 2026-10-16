// C13, F7, C2: the timeline's rows: the dated nodes the level draws, nested as it draws them and
// folded by the ladder, each row's date in plain words, a container as one bar filled by what is
// done, and what a selection or a highlight leaves faded. Over the detail panel's small journey: a
// stage holding a report and its findings, a final meeting milestone, and a decision with no date.
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import type { Timeline } from "./model.ts";
import { chartOf, type ChartOptions } from "./rows.ts";

/** The fixture's level at All: every node, each under its parent. */
const whole = [{ key: "n_stage" }, { key: "n_report", parent: "n_stage" }, { key: "n_findings", parent: "n_stage" }, { key: "n_meeting" }, { key: "n_when" }];
const options: ChartOptions = { drawn: whole, open: () => true, selected: undefined, lit: undefined };

const timeline: Timeline = {
  entries: [
    { node: "n_findings", kind: "deliverable", date: "2026-10-03", origin: "due", overdue: true },
    { node: "n_report", kind: "deliverable", date: "2026-11-02", origin: "pin" },
    { node: "n_meeting", kind: "milestone", date: "2026-11-20", origin: "pin", final: true, shortfall_days: 2 },
  ],
  end: "n_meeting",
};

const keys = (rows: { key: string }[]) => rows.map((row) => row.key);

describe("the rows are the drawn nodes with dates", () => {
  it("opens a stage to its work as one bar over what is done, puts the final milestone last, and lists the undated", () => {
    const chart = chartOf(testView(), timeline, options);
    expect(chart.rows.map((row) => [row.key, row.depth, row.shape])).toEqual([
      ["n_stage", 0, "span"],
      ["n_findings", 1, "tick"],
      ["n_report", 1, "tick"],
      ["n_meeting", 0, "milestone"],
    ]);
    const stage = chart.rows.find((row) => row.key === "n_stage");
    expect([stage?.from, stage?.to]).toEqual(["2026-10-03", "2026-11-02"]);
    expect(chart.undated).toEqual(["n_when"]);
  });

  it("folds a container as asked, which keeps its bar over what it holds", () => {
    const folded = chartOf(testView(), timeline, { ...options, open: (key) => key !== "n_stage" });
    expect(keys(folded.rows)).toEqual(["n_stage", "n_meeting"]);
    expect(folded.rows[0]).toMatchObject({ foldable: true, open: false, from: "2026-10-03", to: "2026-11-02" });
  });

  it("leaves out what the level does not draw: a search's misses, and a hidden container's work goes under nothing", () => {
    const narrowed = chartOf(testView(), timeline, { ...options, drawn: [{ key: "n_stage" }, { key: "n_report", parent: "n_stage" }] });
    expect(keys(narrowed.rows)).toEqual(["n_stage", "n_report"]);
    expect(narrowed.undated).toEqual([]);
    const loose = chartOf(testView(), timeline, { ...options, drawn: [{ key: "n_report" }, { key: "n_meeting" }] });
    expect(loose.rows.map((row) => [row.key, row.depth])).toEqual([["n_report", 0], ["n_meeting", 0]]);
  });

  it("holds open the containers above a selected node, and selects its stand-in when it is not drawn", () => {
    const view = testView();
    const held = chartOf(view, timeline, { ...options, open: () => false, selected: "n_report" });
    expect(keys(held.rows)).toEqual(["n_stage", "n_findings", "n_report", "n_meeting"]);
    const standIn = chartOf(view, timeline, { ...options, drawn: [{ key: "n_stage" }, { key: "n_meeting" }], selected: "n_report" });
    expect(standIn.rows.filter((row) => !row.faded).map((row) => row.key)).toContain("n_stage");
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
    expect(chart.rows.filter((row) => row.float !== undefined).map((row) => [row.key, row.float])).toEqual([["n_report", { from: "2026-10-20", to: "2026-10-30" }]]);
  });

  it("fades the rows a highlight does not light and removes none", () => {
    const lit = chartOf(testView(), timeline, { ...options, lit: new Set(["n_stage", "n_report"]) });
    expect(keys(lit.rows)).toEqual(keys(chartOf(testView(), timeline, options).rows));
    expect(lit.rows.filter((row) => row.faded).map((row) => row.key)).toEqual(["n_findings", "n_meeting"]);
  });

  it("keeps an unlit row faded when the trace reaches it: the lowest opacity wins", () => {
    const chart = chartOf(testView(), timeline, { ...options, selected: "n_report", lit: new Set(["n_stage", "n_report"]) });
    expect(chart.rows.find((row) => row.key === "n_findings")?.faded).toBe(true);
    expect(chart.rows.find((row) => row.key === "n_report")?.faded).toBe(false);
  });
});
