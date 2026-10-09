// One progress rule for a container, whichever screen says it: the graph card, the inspector's
// sentence and the Plan list all read `rollups`, which counts work still in play (not skipped, not
// ruled out) and how much of it is done.
import { describe, expect, it } from "vitest";

import { journeyLooks } from "../canvas/journey.ts";
import { nodeDetail, nodeOf, type Ready } from "../detail/model.ts";
import { plainSentence, sentenceOf } from "../detail/sentence.ts";
import { testView } from "../detail/view.test-support.ts";
import type { DisplayState } from "../status/words.ts";
import { rollups } from "./tree.ts";

/** The test journey with its stage's two children showing `report` and `findings`. */
function showing(report: DisplayState, findings: DisplayState): Ready {
  const view = testView();
  for (const [key, state] of [["n_report", report], ["n_findings", findings]] as const) {
    const found = view.derived.nodes[key];
    if (found !== undefined) {
      view.derived.nodes[key] = { ...found, display_state: state };
    }
  }
  return view;
}

describe("a container's progress", () => {
  it.each([
    { report: "ready", findings: "ready", done: 0, total: 2 },
    { report: "ready", findings: "done", done: 1, total: 2 },
    { report: "done", findings: "done", done: 2, total: 2 },
    { report: "ready", findings: "skipped", done: 0, total: 1 },
    { report: "done", findings: "not_relevant", done: 1, total: 1 },
  ] as const)("is $done of $total when the work is $report and $findings, on the card, in the inspector and in the plan", ({ report, findings, done, total }) => {
    const view = showing(report, findings);
    expect(rollups(view).get("n_stage")).toMatchObject({ done, total });
    const stage = nodeOf(view, "n_stage");
    const detail = nodeDetail(view, "n_stage");
    if (stage === undefined || detail === undefined) {
      throw new Error("the test journey has a stage");
    }
    expect(journeyLooks(view, { ranked: [], mine: [], owned: [] }).card(stage, { key: "n_stage", display_state: "active" }).body).toMatchObject({ kind: "progress", done, total });
    expect(plainSentence(view, sentenceOf(view, detail, { position: undefined, row: undefined }))).toMatch(new RegExp(`^${String(done)} of ${String(total)} done`));
  });
});
