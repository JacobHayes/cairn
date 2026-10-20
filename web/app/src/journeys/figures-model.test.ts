// 2.4, C16: what a list of journeys says of each one: the journey card tells a participant who
// arrives cold how much is still theirs to do, and the index says what needs a look, what is
// theirs, and the one fact a phone row leads with.
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { summaryModel } from "../summary/model.ts";
import type { StatusSummary } from "../summary/model.ts";
import { forYouOf, forYouWords, milestoneOf, troubleOf, troubleWords, yoursOf, type Trouble } from "./figures-model.ts";

const entries = (...nodes: string[]) => nodes.map((node) => ({ node, kinds: ["k_owner"] }));

describe("what is still the viewer's to do (2.4)", () => {
  it("counts their open work, and how much of it is ready to act on", () => {
    expect(yoursOf(testView(), entries("n_report", "n_findings", "n_meeting"))).toEqual({ open: 3, actionable: 1 });
  });

  it("leaves out a container, work not in scope, finished work, and a milestone reached on its own", () => {
    const view = testView();
    const found = view.derived.nodes;
    for (const [key, change] of [["n_findings", { relevance: { value: "not_relevant" as const } }], ["n_meeting", { auto_reached: true, display_state: "done" as const }], ["n_report", { display_state: "done" as const }]] as const) {
      const was = found[key];
      if (was !== undefined) {
        found[key] = { ...was, ...change };
      }
    }
    view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, n_report: { state: "done", provenance: "local" } } };
    expect(yoursOf(view, entries("n_stage", "n_findings", "n_meeting", "n_report", "n_when"))).toEqual({ open: 1, actionable: 0 });
  });
});

/** The test journey's summary with the given lists. */
function model(extra: Partial<StatusSummary>) {
  return summaryModel(testView(), { by_state: {}, by_display_state: { ready: 5 }, remaining: 5, ...extra });
}

describe("what needs a look, worst first", () => {
  it("lists only what is not zero, errors before warnings before an ordinary wait", () => {
    const all = troubleOf(model({ overdue: ["n_findings"], shortfalls: ["n_report", "n_findings"], stale: ["n_report"] }), true);
    expect(all.map((each) => [each.kind, each.count, each.tone])).toEqual([
      ["overdue", 1, "bad"],
      ["shortfall", 2, "bad"],
      ["stale", 1, "warn"],
      ["stalled", 1, undefined],
    ]);
    expect(troubleOf(model({}), false)).toEqual([]);
  });

  it("says each kind in words that agree with its count", () => {
    const words = (kind: Trouble["kind"], count: number) => troubleWords({ kind, count, tone: undefined });
    expect(words("overdue", 2)).toContain("2");
    expect(words("shortfall", 1)).not.toBe(words("shortfall", 2));
    expect(words("stale", 1)).not.toBe(words("stale", 2));
  });
});

describe("the viewer's part in a journey", () => {
  const view = testView();
  const mineOpen = entries("n_report");
  const mineDone = entries("n_stage");

  it("is ready when some of their work can move, else waiting when some is open, else nothing open", () => {
    expect(forYouOf(view, "active", mineOpen, 2)).toEqual({ kind: "ready", count: 2 });
    expect(forYouOf(view, "active", mineOpen, 0)).toEqual({ kind: "waiting" });
    expect(forYouOf(view, "active", mineDone, 0)).toEqual({ kind: "nothing-open" });
    expect(forYouOf(view, "active", [], 0)).toEqual({ kind: "nothing" });
  });

  it("holds no current work in a journey that is not active", () => {
    expect(forYouOf(view, "completed", mineOpen, 2)).toEqual({ kind: "inactive" });
    expect(forYouWords({ kind: "inactive" })).not.toBe(forYouWords({ kind: "nothing" }));
  });
});

describe("the next milestone", () => {
  const today = testView().derived.today;
  const upcoming = (date: string) => model({ upcoming_milestones: [{ node: "n_meeting", date: { date, origin: "pin" } }] });

  it("names it and its date, warning when it is near and error when it has passed", () => {
    expect(milestoneOf(upcoming("2026-11-20"), today)).toMatchObject({ title: "Meeting", tone: undefined });
    expect(milestoneOf(upcoming("2026-10-09"), today)?.tone).toBe("warn");
    expect(milestoneOf(upcoming("2026-10-01"), today)?.tone).toBe("bad");
  });

  it("is nothing when the summary lists none", () => {
    expect(milestoneOf(model({}), today)).toBeUndefined();
  });
});
