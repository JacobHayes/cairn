// C11: with no decision to act on, the walkthrough shows the open decisions in scope, earliest
// first, each with the milestones and dependencies that would unblock it.
import { describe, expect, it } from "vitest";

import { actingView } from "./acting.test-support.ts";
import { actingDecisions, waitingDecisions } from "./waiting.ts";

const waiting = waitingDecisions(actingView());

describe("waitingDecisions", () => {
  it("lists the open decisions in scope off the acting frontier, earliest start first", () => {
    expect(waiting.map((each) => each.decision)).toEqual(["n_wait", "n_inner", "n_later"]);
  });

  it("names a decision's own requirement", () => {
    expect(waiting.find((each) => each.decision === "n_later")?.unblockers).toEqual([{ node: "n_meet", kind: "milestone", via: "explicit" }]);
  });

  it("names the stage opening a decision waits on through its stage, not the stage's own children", () => {
    expect(waiting.find((each) => each.decision === "n_inner")?.unblockers).toEqual([
      { node: "n_meet", kind: "milestone", via: { stage_opening: { group: "n_stage" } }, through: "n_stage" },
    ]);
  });

  it("names a snooze's target", () => {
    expect(waiting.find((each) => each.decision === "n_wait")?.unblockers).toEqual([{ node: "n_work", kind: "deliverable", via: "snooze" }]);
  });
});

describe("actingDecisions", () => {
  it("lists the decisions anyone can act on now, so a filter that hides them is not mistaken for none", () => {
    expect(actingDecisions(actingView())).toEqual(["n_pick"]);
  });
});
