// F6, D8: the inspector says a finished node was done early, and ahead of what: the pieces it
// links (the opening and its stage), the reason its bypass recorded, and no day count ahead of a
// prerequisite that has not finished (its date moves with today).
import { describe, expect, it } from "vitest";

import { nodeDetail, type NodeDerived, type Ready } from "./model.ts";
import { plainSentence, sentenceOf, type Piece } from "./sentence.ts";
import { testView } from "./view.test-support.ts";

type DoneEarly = NonNullable<NodeDerived["dates"]["done_early"]>;

/** The test journey with `key` done early, and the journey changed by `edit`. */
function finishedEarly(key: string, early: DoneEarly, edit: (view: Ready) => void = () => {}): Ready {
  const view = testView();
  const found = view.derived.nodes[key];
  if (found !== undefined) {
    view.derived.nodes[key] = { ...found, display_state: "done", dates: { ...found.dates, done_early: early } };
  }
  view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, [key]: { state: "done", provenance: "local", finished_on: "2026-10-04" } } };
  edit(view);
  return view;
}

function say(view: Ready, key: string): { pieces: Piece[]; plain: string } {
  const detail = nodeDetail(view, key);
  if (detail === undefined) {
    throw new Error(`no ${key}`);
  }
  const pieces = sentenceOf(view, detail, { position: undefined, row: undefined });
  return { pieces, plain: plainSentence(view, pieces) };
}

const links = (pieces: Piece[]) => pieces.flatMap((piece) => (typeof piece === "string" ? [] : [piece.node]));

describe("work done early on purpose", () => {
  it("links the opening and the stage that does not wait for it", () => {
    const early: DoneEarly = { days: 9, date: "2026-11-13", source: { dependency: { node: "n_findings", requires: "n_meeting", via: { stage_opening: { group: "n_stage" } } } } };
    const view = finishedEarly("n_findings", early, (made) => {
      const stage = made.journey.graph.nodes?.find((node) => node.key === "n_stage");
      if (stage !== undefined) {
        stage.gates = false;
      }
    });
    const { pieces, plain } = say(view, "n_findings");
    expect(links(pieces)).toEqual(["n_meeting", "n_stage"]);
    expect(plain).toContain("9 days");
  });

  it("carries the reason its bypass recorded, without the sentence's full stop", () => {
    const early: DoneEarly = { days: 3, date: "2026-10-07", source: { dependency: { node: "n_report", requires: "n_findings", via: "explicit" } } };
    const view = finishedEarly("n_report", early, (made) => {
      made.journey.graph.state = {
        ...made.journey.graph.state,
        overrides: { n_report: { bypass: { guards: ["deps_done"], reason: "The partner sent feedback." } } },
      };
      const findings = made.derived.nodes["n_findings"];
      if (findings !== undefined) {
        made.derived.nodes["n_findings"] = { ...findings, display_state: "done" };
      }
    });
    const { pieces, plain } = say(view, "n_report");
    expect(plain).toContain("The partner sent feedback");
    expect(plain).not.toContain("feedback.\"");
    expect(links(pieces)).toEqual(["n_findings"]);
    expect(plain).toContain("3 days");
  });

  it("says it without a reason when the bypass recorded none", () => {
    const early: DoneEarly = { days: 3, date: "2026-10-07", source: { dependency: { node: "n_report", requires: "n_findings", via: "explicit" } } };
    const view = finishedEarly("n_report", early, (made) => {
      made.journey.graph.state = { ...made.journey.graph.state, overrides: { n_report: { bypass: { guards: ["deps_done"] } } } };
      const findings = made.derived.nodes["n_findings"];
      if (findings !== undefined) {
        made.derived.nodes["n_findings"] = { ...findings, display_state: "done" };
      }
    });
    const { pieces, plain } = say(view, "n_report");
    expect(links(pieces)).toEqual(["n_findings"]);
    expect(plain).toContain("3 days");
    expect(plain).not.toContain("reason");
  });

  it("gives no day count ahead of a prerequisite that has not finished", () => {
    const early: DoneEarly = { days: 5, date: "2026-10-11", source: { dependency: { node: "n_report", requires: "n_findings", via: "explicit" } } };
    const { pieces, plain } = say(finishedEarly("n_report", early), "n_report");
    expect(links(pieces)).toEqual(["n_findings"]);
    expect(plain).not.toMatch(/\d+ days?/);
  });

  it("says a node's own date when that is what it finished ahead of", () => {
    const early: DoneEarly = { days: 4, date: "2026-10-10", source: { not_before: { node: "n_findings" } } };
    const { pieces, plain } = say(finishedEarly("n_findings", early), "n_findings");
    expect(links(pieces)).toEqual([]);
    expect(plain).toContain("4 days");
    expect(plain).toContain("Oct 10");
  });

  it("links the unfinished child a container was finished ahead of, not a not-before date", () => {
    const early: DoneEarly = { days: 5, date: "2026-10-11", source: { containment: { parent: "n_report", child: "n_findings" } } };
    const { pieces, plain } = say(finishedEarly("n_report", early), "n_report");
    expect(links(pieces)).toEqual(["n_findings"]);
    expect(plain).not.toMatch(/not-before|\d+ days?/);
  });
});
