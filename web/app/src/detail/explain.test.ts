// F7: every bound's chain said in the journey's terms, with the pin or rule to edit; F5's
// moves and D4's stale reasons in words naming the nodes they are about.
import { describe, expect, it } from "vitest";

import { constraintText, editTargets, guardFailureText, moveText, namer, sourceText, type Chain } from "./explain.ts";
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

  it("offers nothing to edit for a fact", () => {
    const actual: Chain = { constraints: [], fixed: [{ instant: "created_at", date: "2026-10-01", fixed_by: "actual" }] };
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

  it("names the moves of a resolution and the reasons a node is stale", () => {
    const shift = moveText({ op: "shift_pin", node: "n_report", offset_days: -5 }, name);
    expect(shift).toContain("Report");
    expect(shift).toContain("5");
    expect(moveText({ op: "answer", decision: "n_when", value: { date: "2026-11-25" } }, name)).toContain("2026-11-25");
    expect(guardFailureText({ open_dependency: "n_findings" }, name)).toContain("Findings");
  });
});
