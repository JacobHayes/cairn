// C13: what a container's snooze would hold back is the engine's own count of its open work.
import { describe, expect, it } from "vitest";

import { openUnder } from "./snooze.ts";
import { testView } from "./view.test-support.ts";

describe("openUnder", () => {
  it("is the unfinished in-scope work the engine counted beneath a container, and nothing for a node with no progress", () => {
    const view = testView();
    const stage = view.derived.nodes["n_stage"];
    if (stage === undefined) {
      throw new Error("the test journey has a stage");
    }
    view.derived.nodes["n_stage"] = { ...stage, progress: { done: 1, total: 4, overdue: 0, decisions_needed: 0 } };
    expect(openUnder(view, "n_stage")).toBe(3);
    expect(openUnder(view, "n_meeting")).toBe(0);
  });
});
