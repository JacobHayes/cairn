// The journey card's Stages list: a row per stage, with what its card says of it.
import { describe, expect, it } from "vitest";

import { testView } from "../detail/view.test-support.ts";
import { stagesOf } from "./stages.ts";

/** The test journey with `n_stage` due on 2026-11-20 and a second stage, a group inside a stage, and the journey's other nodes unchanged. */
function viewWithStages() {
  const view = testView();
  const nodes = view.journey.graph.nodes ?? [];
  nodes.push({ key: "n_later", id: "later", kind: "group", title: "Later" }, { key: "n_inner", id: "inner", parent: "n_stage", kind: "group", title: "Inner" });
  const stage = view.derived.nodes["n_stage"];
  if (stage === undefined) {
    throw new Error("the test journey has a stage");
  }
  view.derived.nodes["n_stage"] = { ...stage, dates: { due: { date: "2026-11-20", chain: { constraints: [], fixed: [{ instant: "n_stage.finish", date: "2026-11-20", fixed_by: "pin" }] } } } };
  view.derived.nodes["n_later"] = { ...stage, display_state: "done", progress: { done: 4, total: 4, overdue: 0, decisions_needed: 0 } };
  return view;
}

describe("the stages of a journey", () => {
  it("lists the top-level groups, not the groups inside them, the earlier due date first and the undated after", () => {
    const view = viewWithStages();
    expect(stagesOf(view).map((row) => row.key)).toEqual(["n_stage", "n_later"]);
    const stage = view.derived.nodes["n_stage"];
    if (stage !== undefined) {
      view.derived.nodes["n_later"] = { ...stage, dates: { due: { date: "2026-11-10", chain: { constraints: [], fixed: [] } } } };
    }
    expect(stagesOf(view).map((row) => row.key)).toEqual(["n_later", "n_stage"]);
    // A stage with no due date at all comes after both.
    view.derived.nodes["n_undated"] = { ...(stage as NonNullable<typeof stage>), dates: {} };
    view.journey.graph.nodes = [{ key: "n_undated", id: "undated", kind: "group", title: "Undated" }, ...(view.journey.graph.nodes ?? [])];
    expect(stagesOf(view).map((row) => row.key)).toEqual(["n_later", "n_stage", "n_undated"]);
  });

  it("carries each stage's state and done count, and the due date only while it is open", () => {
    const [open, finished] = stagesOf(viewWithStages());
    expect([open?.displayState, open?.done, open?.total]).toEqual(["active", 0, 2]);
    expect(open?.due).toBeDefined();
    expect([finished?.displayState, finished?.done, finished?.total, finished?.due]).toEqual(["done", 4, 4, undefined]);
  });

  it("says a due date is late once it has gone by, and nothing once the stage is finished", () => {
    const view = viewWithStages();
    view.derived.today = "2026-12-01";
    const [open, finished] = stagesOf(view);
    expect([open?.dueInk, finished?.dueInk]).toEqual(["ink-bad", undefined]);
  });

  it("is empty for a journey without stages", () => {
    const view = testView();
    view.journey.graph.nodes = (view.journey.graph.nodes ?? []).filter((node) => node.kind !== "group").map((node) => ({ ...node, parent: null }));
    expect(stagesOf(view)).toEqual([]);
  });
});
