// C8's node detail composed in the tab from the document and its derive, and D1's moves.
import { describe, expect, it } from "vitest";

import { flagsOf, isBlocked, movesFrom, nodeDetail, transition, unansweredOf, type Move, type NodeDerived, type NodeKind, type State } from "./model.ts";
import { testView } from "./view.test-support.ts";

describe("nodeDetail", () => {
  it("composes a node's path, record, pin, annotations, and local edits from the document", () => {
    const detail = nodeDetail(testView(), "n_report");
    expect(detail?.path).toBe("stage/report");
    expect(detail?.ancestors.map((node) => node.key)).toEqual(["n_stage"]);
    expect(detail?.record).toMatchObject({ state: "active", provenance: "from_route" });
    expect(detail?.pin).toBe("2026-11-02");
    expect(detail?.annotations.map((annotation) => annotation.body.key)).toEqual(["a_link"]);
    expect(detail?.localEdits).toEqual([{ field: "title" }]);
    expect(detail?.fedBy).toBeUndefined();
  });

  it("lists a container's children with their states, initial where none is stored", () => {
    const children = nodeDetail(testView(), "n_stage")?.children;
    expect(children?.map((child) => [child.key, child.state])).toEqual([
      ["n_report", "active"],
      ["n_findings", "todo"],
    ]);
  });

  it("names the decision that feeds a milestone's pin (E3)", () => {
    expect(nodeDetail(testView(), "n_meeting")?.fedBy?.key).toBe("n_when");
  });

  it("is undefined for a node the journey does not hold", () => {
    expect(nodeDetail(testView(), "n_absent")).toBeUndefined();
  });
});

describe("movesFrom (D1)", () => {
  const cases: [NodeKind, State, Move[]][] = [
    ["deliverable", "todo", ["start", "complete", "skip"]],
    ["action", "active", ["stop", "complete", "skip"]],
    ["deliverable", "done", ["reopen"]],
    ["decision", "open", ["skip"]],
    ["decision", "decided", ["reopen"]],
    ["milestone", "pending", ["reach", "skip"]],
    ["milestone", "reached", ["reopen"]],
    ["group", "derived", ["skip"]],
    ["group", "skipped", ["reopen"]],
  ];
  it.each(cases)("a %s in %s can %j", (kind, state, moves) => {
    expect(movesFrom(kind, state)).toEqual(moves);
  });

  it("carries a skip's reason in its mutation", () => {
    expect(transition("n_a", "skip", "out of scope")).toEqual({
      op: "transition",
      node: "n_a",
      transition: { skip: { reason: "out of scope" } },
    });
    expect(transition("n_a", "complete")).toEqual({ op: "transition", node: "n_a", transition: "complete" });
  });
});

describe("flags (D3)", () => {
  it("flags only real attention: work started before its gates are met, not what the display state says", () => {
    const view = testView();
    const flags = (key: string, state: State) => {
      const derived = view.derived.nodes[key];
      if (derived === undefined) {
        throw new Error(`no ${key}`);
      }
      return flagsOf(derived, state).map((each) => each.flag);
    };
    expect(flags("n_report", "active")).toEqual(["started early"]);
    expect(flags("n_report", "todo")).toEqual([]);
    expect(flags("n_findings", "todo")).toEqual([]);
    expect(flags("n_stage", "active")).toEqual([]);
  });

  it("does not call a finished node blocked by what it still holds back", () => {
    const derived = testView().derived.nodes["n_report"];
    expect(derived && isBlocked({ ...derived, display_state: "done" })).toBe(false);
    expect(derived && isBlocked({ ...derived, relevance: { value: "not_relevant" }, display_state: "not_relevant" })).toBe(false);
  });
});

describe("unansweredOf (Gating, D4)", () => {
  /** The test view with `relevance` on some nodes and `states` stored. */
  const viewWith = (relevance: Record<string, NodeDerived["relevance"]>, states: Record<string, State> = {}) => {
    const view = testView();
    for (const [key, value] of Object.entries(relevance)) {
      const found = view.derived.nodes[key];
      if (found !== undefined) {
        view.derived.nodes[key] = { ...found, relevance: value };
      }
    }
    const stored = Object.fromEntries(Object.entries(states).map(([key, state]) => [key, { state, provenance: "local" as const }]));
    view.journey.graph.state = { ...view.journey.graph.state, nodes: stored };
    return view;
  };

  it("names the open, relevant decisions an undecided node waits on", () => {
    const view = viewWith({ n_findings: { value: "undecided", decisions: ["n_when"] } });
    expect(unansweredOf(view, "n_findings")).toEqual(["n_when"]);
  });

  it("names an undecided ancestor's decisions too", () => {
    const view = viewWith({
      n_stage: { value: "undecided", decisions: ["n_when"] },
      n_findings: { value: "undecided", condition_on: "n_stage", decisions: ["n_when"] },
    });
    expect(unansweredOf(view, "n_findings")).toEqual(["n_when"]);
  });

  it("names nothing for a decided decision or a node that is not undecided", () => {
    const decided = viewWith({ n_findings: { value: "undecided", decisions: ["n_when"] } }, { n_when: "decided" });
    expect(unansweredOf(decided, "n_findings")).toEqual([]);
    expect(unansweredOf(testView(), "n_findings")).toEqual([]);
  });
});
