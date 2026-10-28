// D8: what "finished" means for a display state.
import { describe, expect, it } from "vitest";

import { isFinished } from "./state.ts";

describe("isFinished", () => {
  it("is true for done and skipped display states, and for no other", () => {
    expect(isFinished("done")).toBe(true);
    expect(isFinished("skipped")).toBe(true);
    for (const state of ["ready", "active", "blocked", "conditional", "scheduled", "snoozed", "not_relevant"] as const) {
      expect(isFinished(state)).toBe(false);
    }
  });
});
