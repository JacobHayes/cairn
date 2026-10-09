// C13, F7: the timeline's rows: the containment tree folded by the detail ladder, each row's date
// in plain words, a container as one bar filled by what is done, and what a selection or a filter
// leaves faded. Over the detail panel's small journey: a stage holding a report and its findings,
// a final meeting milestone, and a decision with no date.
import { describe, expect, it } from "vitest";

import type { Ready } from "../detail/model.ts";
import { testView } from "../detail/view.test-support.ts";
import type { DisplayState } from "../status/words.ts";
import type { Timeline } from "./model.ts";
import { chartOf, type ChartOptions, type Narrowing } from "./rows.ts";

const none: Narrowing = { decisions: false, kinds: [], text: "", mine: undefined };
const options: ChartOptions = { detail: "stages", flipped: new Set(), selected: undefined, narrow: none };

const timeline: Timeline = {
  entries: [
    { node: "n_findings", kind: "deliverable", date: "2026-10-03", origin: "due", overdue: true },
    { node: "n_report", kind: "deliverable", date: "2026-11-02", origin: "pin" },
    { node: "n_meeting", kind: "milestone", date: "2026-11-20", origin: "pin", final: true, shortfall_days: 2 },
  ],
  end: "n_meeting",
};

/** `view` with node `key` showing `state`. */
function showing(view: Ready, key: string, state: DisplayState): Ready {
  const node = view.derived.nodes[key];
  if (node === undefined) {
    throw new Error(`no node ${key}`);
  }
  return { ...view, derived: { ...view.derived, nodes: { ...view.derived.nodes, [key]: { ...node, display_state: state } } } };
}

const keys = (rows: { key: string }[]) => rows.map((row) => row.key);

describe("the rows follow the containment tree", () => {
  it("opens the current stage to its work as one bar over what is done, puts the final milestone last, and lists the undated", () => {
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

  it("folds a stage that is not current, and again when its fold is flipped", () => {
    const quiet = showing(showing(testView(), "n_stage", "ready"), "n_report", "ready");
    quiet.derived.acting_frontier = [];
    const folded = chartOf(quiet, timeline, options);
    expect(keys(folded.rows)).toEqual(["n_stage", "n_meeting"]);
    expect(folded.rows[0]).toMatchObject({ foldable: true, open: false });
    expect(keys(chartOf(quiet, timeline, { ...options, flipped: new Set(["n_stage"]) }).rows)).toEqual(["n_stage", "n_findings", "n_report", "n_meeting"]);
  });
});

describe("a selection and the toolbar's chips fade what they do not reach", () => {
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

  it("fades what the kinds, the search and Mine exclude, keeping a stage that holds a match", () => {
    const faded = (narrow: Narrowing) => chartOf(testView(), timeline, { ...options, narrow }).rows.filter((row) => row.faded).map((row) => row.key);
    expect(faded({ ...none, kinds: ["milestone"] })).toEqual(["n_stage", "n_findings", "n_report"]);
    expect(faded({ ...none, text: "find" })).toEqual(["n_report", "n_meeting"]);
    expect(faded({ ...none, mine: new Set(["n_report"]) })).toEqual(["n_findings", "n_meeting"]);
  });
});
