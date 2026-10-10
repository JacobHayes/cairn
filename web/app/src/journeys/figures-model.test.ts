// 2.4: the journey card tells a participant who arrives cold how much is still theirs to do.
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { yoursOf } from "./figures-model.ts";

const entries = (...nodes: string[]) => nodes.map((node) => ({ node, kinds: ["k_owner"] }));

describe("what is still the viewer's to do (2.4)", () => {
  it("counts their open work, and how much of it is ready to act on", () => {
    expect(yoursOf(testView(), entries("n_report", "n_findings", "n_meeting"))).toEqual({ open: 3, actionable: 1 });
  });

  it("leaves out a container, work not in scope, finished work, and a milestone reached on its own", () => {
    const view = testView();
    const found = view.derived.nodes;
    for (const [key, change] of [["n_findings", { relevance: { value: "not_relevant" as const } }], ["n_meeting", { auto_reached: true }]] as const) {
      const was = found[key];
      if (was !== undefined) {
        found[key] = { ...was, ...change };
      }
    }
    view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, n_report: { state: "done", provenance: "local" } } };
    expect(yoursOf(view, entries("n_stage", "n_findings", "n_meeting", "n_report", "n_when"))).toEqual({ open: 1, actionable: 0 });
  });
});
