// D8, Gating: what "open" and "finished" mean for a node, read from the engine's values.
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { isFinished, isOpen } from "./state.ts";

describe("isFinished", () => {
  it("is true for done and skipped display states, and for no other", () => {
    expect(isFinished("done")).toBe(true);
    expect(isFinished("skipped")).toBe(true);
    for (const state of ["ready", "active", "blocked", "conditional", "scheduled", "snoozed", "not_relevant"] as const) {
      expect(isFinished(state)).toBe(false);
    }
  });
});

describe("isOpen", () => {
  const derived = testView().derived.nodes["n_report"];
  if (derived === undefined) {
    throw new Error("the test journey has n_report");
  }

  it("is true for relevant and undecided work still to do", () => {
    expect(isOpen({ ...derived, display_state: "active" })).toBe(true);
    expect(isOpen({ ...derived, relevance: { value: "undecided", decisions: [], unanswered: [] }, display_state: "blocked" })).toBe(true);
  });

  it("is false for finished work, for work ruled out, and for work that waits on an unanswered decision to apply at all", () => {
    expect(isOpen({ ...derived, display_state: "done" })).toBe(false);
    expect(isOpen({ ...derived, display_state: "skipped" })).toBe(false);
    expect(isOpen({ ...derived, relevance: { value: "not_relevant" }, display_state: "not_relevant" })).toBe(false);
    expect(isOpen({ ...derived, relevance: { value: "not_relevant" }, display_state: "conditional" })).toBe(false);
  });

  it("is false for a node the engine did not derive", () => {
    expect(isOpen(undefined)).toBe(false);
  });
});
