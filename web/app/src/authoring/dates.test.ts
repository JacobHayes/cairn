// A8 and F4 against the engine: every source a date rule editor offers is one the engine
// measures from, a rule round-trips through its parts, and a stage's bounds are offered from
// the milestones outside it.
import type { InBrowserHost } from "@cairn/wasm";
import { beforeAll, describe, expect, it } from "vitest";

import { boundOptions, CREATED_AT, partsOf, ruleOf, sourceOptions } from "./dates.ts";
import { applyToDraft, seeded } from "./engine.test-support.ts";
import { treeOf, type Graph, type GraphNode } from "./graph.ts";

describe("date rules (A8)", () => {
  let host: InBrowserHost;
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
      const rule = ruleOf({ direction: "before", sources: [source], offset: 2 });
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
  it("is no number, not 0, in the parts, in the rule, and after a draft is kept as JSON", () => {
    const rule = ruleOf({ direction: "before", sources: ["n_kickoff"], offset: Number.NaN });
    expect(Number.isNaN(partsOf(rule).offset)).toBe(true);
    expect(Number.isNaN(partsOf(JSON.parse(JSON.stringify(rule)) as typeof rule).offset)).toBe(true);
    expect(partsOf({ before: "n_kickoff" }).offset).toBe(0);
  });
});

describe("stage bounds (F4)", () => {
  it("offers the milestones outside the stage first, and those inside it marked", () => {
    const nodes: GraphNode[] = [
      { key: "n_stage", id: "stage", kind: "group", title: "Stage" },
      { key: "n_inside", id: "inside", kind: "milestone", title: "Inside", parent: "n_stage" },
      { key: "n_outside", id: "outside", kind: "milestone", title: "Outside" },
      { key: "n_work", id: "work", kind: "action", title: "Work" },
    ];
    expect(boundOptions(treeOf({ nodes }), "n_stage", "opens_at").map(({ node, inside }) => [node.key, inside])).toEqual([
      ["n_outside", false],
      ["n_inside", true],
    ]);
  });
});
