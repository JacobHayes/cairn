// C18: the status summary as a reader sees it: counts by state in lifecycle order, what is in
// scope and what remains, the overdue with how late, the short with by how much, the stale
// with why, the upcoming milestones with their dates and owners, and the open decisions with
// their owners.
import { describe, expect, it } from "vitest";

import type { Ready } from "../detail/model.ts";
import { testView } from "../detail/view.test-support.ts";
import { STATE_ORDER, summaryModel, type StatusSummary } from "./model.ts";

/** The test journey with the findings overdue since 2026-10-01, the report stale and short, and the meeting owned. */
function summaryJourney(): Ready {
  const view = testView();
  const nodes = view.derived.nodes;
  const findings = nodes["n_findings"];
  const report = nodes["n_report"];
  const meeting = nodes["n_meeting"];
  if (findings === undefined || report === undefined || meeting === undefined) {
    throw new Error("the test journey lost a node");
  }
  findings.overdue = true;
  findings.dates = { due: { date: "2026-10-01", chain: { constraints: [] } } };
  report.stale = ["missing_artifact", { open_dependency: "n_findings" }];
  report.dates.shortfall = { chain: { constraints: [] }, shortfall_days: 3 };
  meeting.participations = { k_owner: { entities: ["e_two"], origin: "explicit" } };
  return view;
}

const projected: StatusSummary = {
  by_state: { decided: 1, active: 1, todo: 1, pending: 1, derived: 1 },
  remaining: 4,
  overdue: ["n_findings"],
  shortfalls: ["n_report"],
  stale: ["n_report"],
  upcoming_milestones: [{ node: "n_meeting", date: { date: "2026-11-20", origin: "pin" } }],
  open_decisions: [{ node: "n_when", owners: ["e_one"] }],
};

describe("C18: the status summary", () => {
  it("counts by state in lifecycle order, leaving out the states with none", () => {
    const model = summaryModel(summaryJourney(), projected);
    const order = model.byState.map((each) => STATE_ORDER.indexOf(each.state));
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(Object.fromEntries(model.byState.map((each) => [each.state, each.count]))).toEqual(projected.by_state);
  });

  it("counts what is in scope as every state's count, and what remains as projected", () => {
    const model = summaryModel(summaryJourney(), projected);
    expect([model.inScope, model.remaining]).toEqual([5, 4]);
  });

  it("lists the overdue with their due dates and how late, today being the derive's", () => {
    const view = summaryJourney();
    const [findings] = summaryModel(view, projected).overdue;
    expect(view.derived.today).toBe("2026-10-06");
    expect(findings).toEqual({ key: "n_findings", title: "Findings", due: "2026-10-01", lateDays: 5 });
  });

  it("lists the short with their days and the stale with why", () => {
    const model = summaryModel(summaryJourney(), projected);
    expect(model.shortfalls).toEqual([{ key: "n_report", title: "Report", days: 3 }]);
    expect(model.stale.map((each) => [each.key, each.reasons.length])).toEqual([["n_report", 2]]);
  });

  it("lists upcoming milestones with date, origin, and owner, and open decisions with owners by name", () => {
    const model = summaryModel(summaryJourney(), projected);
    expect(model.upcoming).toEqual([{ key: "n_meeting", title: "Meeting", date: "2026-11-20", origin: "pin", owners: ["Person Two"] }]);
    expect(model.openDecisions).toEqual([{ key: "n_when", title: "Meeting date", owners: ["Person One"] }]);
  });

  it("lists nothing where the projection lists nothing", () => {
    const model = summaryModel(summaryJourney(), { by_state: {}, remaining: 0 });
    expect([model.byState, model.overdue, model.shortfalls, model.stale, model.upcoming, model.openDecisions]).toEqual([[], [], [], [], [], []]);
    expect(model.inScope).toBe(0);
  });
});
