// Priority: gravity splits into its own weight, what applies downstream, and what waits on a
// decision at the discount.
import { describe, expect, it } from "vitest";

import { gravityParts } from "./rank.ts";
import { testView } from "./view.test-support.ts";

describe("gravityParts", () => {
  it("splits the contributors into what applies and what waits on each open decision", () => {
    const view = testView();
    const meeting = view.derived.nodes["n_meeting"];
    if (meeting !== undefined) {
      view.derived.nodes["n_meeting"] = { ...meeting, relevance: { value: "undecided", decisions: ["n_when"] } };
    }
    const parts = gravityParts(view, 6.5, [
      { node: "n_report", score: 2 },
      { node: "n_meeting", score: 1.5 },
    ]);
    expect(parts).toEqual({
      own: 3,
      applies: { nodes: 1, adds: 2 },
      conditional: [{ decision: "n_when", nodes: 1, adds: 1.5 }],
    });
  });
});
