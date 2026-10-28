// F7: every bound's chain said in the journey's terms, with the pin or rule to edit; D4's
// stale reasons in words naming the nodes they are about.
import { describe, expect, it } from "vitest";

import { constraintText, editTargets, staleText, namer, sourceText, type Chain } from "./explain.ts";
import { testView } from "./view.test-support.ts";

const view = testView();
const name = namer(view);
const due = (key: string): Chain => {
  const chain = view.derived.nodes[key]?.dates.due?.chain;
  if (chain === undefined) {
    throw new Error(`${key} has no due`);
  }
  return chain;
};

describe("editTargets (F7)", () => {
  it("names the pin a pinned date comes from", () => {
    expect(editTargets(due("n_report"), view)).toEqual([{ edit: "pin", node: "n_report" }]);
  });

  it("names the pin and every rule along a derived chain, each once", () => {
    expect(editTargets(due("n_findings"), view)).toEqual([
      { edit: "pin", node: "n_report" },
      { edit: "requirement", node: "n_report", requires: "n_findings" },
      { edit: "estimate", node: "n_report" },
    ]);
  });

  it("routes a fed milestone's pin through its decision (E3)", () => {
    expect(editTargets(due("n_meeting"), view)).toEqual([{ edit: "answer", decision: "n_when" }]);
  });

  it("points a dependency a node inherits from its container at the container's requirement, not the node's", () => {
    const inherited: Chain = { constraints: [{ before: "n_findings.finish", after: "n_report.start", days: 0, source: "dependency:n_report:n_findings:inherited:n_stage" }], fixed: [] };
    expect(editTargets(inherited, view)).toEqual([{ edit: "requirement", node: "n_stage", requires: "n_findings" }]);
  });

  it("offers no requirement to edit for a dependency that comes from containment or a decision's condition", () => {
    const from = (source: string): Chain => ({ constraints: [{ before: "n_findings.finish", after: "n_report.start", days: 0, source }], fixed: [] });
    expect(editTargets(from("dependency:n_report:n_findings:containment"), view)).toEqual([]);
    expect(editTargets(from("dependency:n_report:n_pick:condition:n_pick"), view)).toEqual([]);
  });

  it("offers nothing to edit for a fact", () => {
    const actual: Chain = { constraints: [], fixed: [{ instant: "created", date: "2026-10-01", fixed_by: "actual" }] };
    expect(editTargets(actual, view)).toEqual([]);
  });
});

describe("explanation words", () => {
  it("names the nodes a constraint and its source are about, by title", () => {
    const [requirement, estimate] = due("n_findings").constraints;
    expect(requirement && constraintText(requirement, name)).toContain("Findings");
    expect(estimate && constraintText(estimate, name)).toContain("3");
    expect(estimate && sourceText(estimate.source, name)).toContain("Report");
  });

  it("names the reasons a node is stale", () => {
    expect(staleText({ reopened: "n_findings" }, name, (id) => id)).toContain("Findings");
    expect(staleText({ unticked: "load" }, name, () => "Load test")).toContain("Load test");
  });
});
