// A8 against the engine: every source a date rule editor offers is one the engine measures
// from, and a rule round-trips through its parts.
import { beforeAll, describe, expect, it } from "vitest";

import { CREATED_AT, partsOf, ruleOf, sourceOptions } from "./dates.ts";
import { applyToDraft, seeded, type Fixtures } from "./engine.test-support.ts";
import { treeOf, type Graph, type GraphNode } from "./graph.ts";

describe("date rules (A8)", () => {
  let host: Fixtures;
  let graph: Graph;
  beforeAll(async () => {
    host = await seeded();
    graph = host.routeVersion("vendor-evaluation", 1).graph;
  });

  it("offers `created_at`, the milestones, and the date decisions, each accepted as a source", () => {
    // A lone action, so no other constraint can make a rule on it contradictory.
    const lone: GraphNode = { key: "n_lone", id: "lone", kind: "action", title: "Lone" };
    const options = sourceOptions(treeOf(graph), lone.key, "due_by");
    expect(options[0]?.source).toBe(CREATED_AT);
    expect(options.map((option) => option.source)).toEqual(expect.arrayContaining(["n_kickoff", "n_decision_meeting", "n_meeting_date"]));
    for (const { source } of options) {
      const rule = ruleOf({ direction: "before", sources: [source], offset: "2" });
      const outcome = applyToDraft(host.engine, host.deployment(), graph, [{ op: "add_node", node: { ...lone, due_by: rule } }]);
      expect([source, outcome.accepted]).toEqual([source, true]);
    }
  });

  it("never offers the node itself, or a node that is neither a milestone nor a date decision", () => {
    const sources = new Set(sourceOptions(treeOf(graph), "n_kickoff", "due_by").map((option) => option.source));
    expect(sources.has("n_kickoff")).toBe(false);
    expect(sources.has("n_purpose")).toBe(false);
    expect(sources.has("n_plan")).toBe(false);
  });

  it("round-trips a rule through its parts, one source bare and several as a list", () => {
    for (const rule of [{ before: "n_kickoff", offset: 14 }, { after: ["n_kickoff", CREATED_AT] }]) {
      expect(ruleOf(partsOf(rule))).toEqual(rule);
    }
  });
});

describe("a blank offset", () => {
  it("stays blank as text in the parts, and makes a rule with no number, not 0", () => {
    const parts = { direction: "before" as const, sources: ["n_kickoff"], offset: "" };
    expect(partsOf(ruleOf(parts)).offset).not.toBe("0");
    expect(ruleOf(parts).offset).toBeNaN();
    expect(partsOf({ before: "n_kickoff" }).offset).toBe("0");
  });
});
