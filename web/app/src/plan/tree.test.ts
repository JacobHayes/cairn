// One progress figure for a container, whichever screen says it: the graph card and the
// inspector's sentence both read the engine's `progress` on the node. How it is counted (work
// still in play, not skipped, not ruled out) is the engine's, tested there.
import { describe, expect, it } from "vitest";

import { journeyLooks } from "../canvas/journey.ts";
import { nodeDetail, nodeOf, type Ready } from "../detail/model.ts";
import { plainSentence, sentenceOf } from "../detail/sentence.ts";
import { testView } from "../detail/view.test-support.ts";
import { treeOf } from "./tree.ts";

/** The test journey with its stage's engine progress set to `done` of `total`. */
function showing(done: number, total: number): Ready {
  const view = testView();
  const found = view.derived.nodes["n_stage"];
  if (found !== undefined) {
    view.derived.nodes["n_stage"] = { ...found, progress: { done, total, overdue: 0, decisions_needed: 0 } };
  }
  return view;
}

describe("a container's progress", () => {
  it.each([
    { done: 0, total: 2 },
    { done: 1, total: 2 },
    { done: 2, total: 2 },
    { done: 0, total: 1 },
  ])("is $done of $total on the card and in the inspector when the engine says so", ({ done, total }) => {
    const view = showing(done, total);
    const stage = nodeOf(view, "n_stage");
    const detail = nodeDetail(view, "n_stage");
    if (stage === undefined || detail === undefined) {
      throw new Error("the test journey has a stage");
    }
    expect(journeyLooks(view, { mine: [], owned: [] }).card(stage, { key: "n_stage", display_state: "active" }).body).toMatchObject({ kind: "progress", done, total });
    expect(plainSentence(view, sentenceOf(view, detail, { position: undefined, row: undefined }))).toMatch(new RegExp(`^${String(done)} of ${String(total)} done`));
  });
});

describe("the order of siblings", () => {
  it("puts the final milestone last whatever its date, and an ordinary milestone by its date", () => {
    const orderOf = (final: boolean) => {
      const view = testView();
      const meeting = view.derived.nodes["n_meeting"];
      const node = view.journey.graph.nodes?.find((each) => each.key === "n_meeting");
      if (meeting === undefined || node === undefined) {
        throw new Error("the test journey has the meeting");
      }
      // Due before everything else in the journey.
      view.derived.nodes["n_meeting"] = { ...meeting, dates: { ...meeting.dates, due: { date: "2000-01-01", chain: { constraints: [], fixed: [] } } } };
      node.final = final;
      return treeOf(view).roots;
    };
    expect(orderOf(false)[0]).toBe("n_meeting");
    expect(orderOf(true).at(-1)).toBe("n_meeting");
  });
});
