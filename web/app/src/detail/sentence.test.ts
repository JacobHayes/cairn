// C8, D4, D8: the inspector's sentence for finished work and decisions: the day it finished,
// the answer a decision holds, the node a stale reason names, and the decision an undecided
// node depends on, each as a piece the component links.
import { OWNER_KIND } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { nodeDetail, type NodeDerived, type Ready } from "./model.ts";
import { plainSentence, sentenceOf, type Piece } from "./sentence.ts";
import { testView } from "./view.test-support.ts";

/** The test journey with `key` stored `state` on Oct 4 and its derived values changed. */
function finished(key: string, state: "done" | "decided", derived: Partial<NodeDerived>): Ready {
  const view = testView();
  const found = view.derived.nodes[key];
  if (found !== undefined) {
    view.derived.nodes[key] = { ...found, display_state: "done", ...derived };
  }
  view.journey.graph.state = { ...view.journey.graph.state, nodes: { ...view.journey.graph.state?.nodes, [key]: { state, finished_on: "2026-10-04" } } };
  return view;
}

function pieces(view: Ready, key: string): Piece[] {
  const detail = nodeDetail(view, key);
  if (detail === undefined) {
    throw new Error(`no ${key}`);
  }
  return sentenceOf(view, detail, { position: undefined, row: undefined });
}

describe("the sentence of finished work", () => {
  it("says the day work finished, and links the reopened node a stale reason names", () => {
    const clear = finished("n_findings", "done", {});
    expect(plainSentence(clear, pieces(clear, "n_findings"))).toContain("Oct 4");
    expect(pieces(clear, "n_findings")).not.toContainEqual({ node: "n_report" });
    const stale = finished("n_findings", "done", { stale: [{ reopened: "n_report" }] });
    expect(pieces(stale, "n_findings")).toContainEqual({ node: "n_report" });
  });

  it("says a decision's answer, and links the decision an undecided node depends on", () => {
    const decided = finished("n_when", "decided", {});
    expect(plainSentence(decided, pieces(decided, "n_when"))).toContain("Nov 20");
    const undecided = finished("n_findings", "done", { relevance: { value: "undecided", unanswered: ["n_pick"] } });
    expect(pieces(undecided, "n_findings")).toContainEqual({ node: "n_pick" });
  });
});

describe("the sentence of waiting work", () => {
  it("links what it waits on and names that node's owner", () => {
    const view = testView();
    const report = view.derived.nodes["n_report"];
    const findings = view.derived.nodes["n_findings"];
    if (report === undefined || findings === undefined) {
      throw new Error("no n_report");
    }
    view.derived.nodes["n_report"] = { ...report, display_state: "blocked", waits_on: { entries: [{ node: "n_findings", via: "explicit" }], total: 1 } };
    view.derived.nodes["n_findings"] = { ...findings, participations: { [OWNER_KIND.key]: { identities: ["who_two"], origin: "explicit" } } };
    const said = pieces(view, "n_report");
    expect(said).toContainEqual({ node: "n_findings" });
    expect(plainSentence(view, said)).toContain("Person Two");
  });
});
