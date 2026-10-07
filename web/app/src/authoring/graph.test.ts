// A3 against the engine: an edge the editor refuses is one the engine rejects, and one it lets
// an author draw the engine accepts; A2: a node moves only into a container outside itself.
import type { Schema } from "@cairn/client";
import type { InBrowserHost } from "@cairn/wasm";
import { beforeAll, describe, expect, it } from "vitest";

import { applyToDraft, seeded } from "./engine.test-support.ts";
import { edgeRefusal, moveTargets, treeOf, type Graph } from "./graph.ts";
import { mintKey, slugOf, uniqueId } from "./keys.ts";

describe("edgeRefusal (A3)", () => {
  let host: InBrowserHost;
  let graph: Graph;
  beforeAll(async () => {
    host = await seeded();
    graph = host.routeVersion("vendor-evaluation", 1).graph;
  });

  const edges: [string, string, string, Schema<"ViolationCode"> | undefined][] = [
    ["a child on its group", "n_access", "n_setup", "edge_to_ancestor_or_descendant"],
    ["a group on its grandchild", "n_setup", "n_plan_draft", "edge_to_ancestor_or_descendant"],
    ["a node on itself", "n_plan", "n_plan", "dependency_cycle"],
    ["two nodes containment does not relate", "n_findings", "n_kickoff", undefined],
    ["across groups at different depths", "n_final_report", "n_plan_review", undefined],
  ];

  it.each(edges)("%s: refused exactly when the engine rejects it", (_, node, requires, code) => {
    const refusal = edgeRefusal(treeOf(graph), node, requires);
    const outcome = applyToDraft(host.engine, host.deployment(), graph, [{ op: "add_edge", edge: { node, requires } }]);
    expect([refusal !== undefined, outcome.accepted ? undefined : outcome.codes]).toEqual([code !== undefined, code === undefined ? undefined : [code]]);
  });

  it("refuses an edge the node already has, which would change nothing", () => {
    expect(edgeRefusal(treeOf(graph), "n_plan", "n_access")).toBeDefined();
  });
});

describe("moveTargets (A2)", () => {
  it("offers containers outside the node, never a leaf, the node, or anything inside it", () => {
    const nodes: Schema<"Node">[] = [
      { key: "n_a", id: "a", kind: "group", title: "A" },
      { key: "n_b", id: "b", kind: "deliverable", title: "B", parent: "n_a" },
      { key: "n_c", id: "c", kind: "action", title: "C", parent: "n_b" },
      { key: "n_d", id: "d", kind: "decision", title: "D", prompt: "?", answer_type: "boolean" },
      { key: "n_e", id: "e", kind: "group", title: "E" },
    ];
    expect(moveTargets(treeOf({ nodes }), "n_b").map((node) => node.key)).toEqual(["n_a", "n_e"]);
  });
});

describe("ids and keys (Identity and references)", () => {
  it("makes a sibling-unique slug from a title", () => {
    expect(slugOf("Review the partner's criteria!")).toBe("review-the-partner-s-criteria");
    expect(slugOf("???")).toBe("node");
    expect(uniqueId("plan", new Set(["plan", "plan-2"]))).toBe("plan-3");
    expect(uniqueId("plan", new Set())).toBe("plan");
  });

  it("mints a key with its type's prefix", () => {
    expect(mintKey("n_", () => "abc123")).toBe("n_abc123");
    expect(mintKey("r_")).toMatch(/^r_[a-z0-9]+$/);
  });
});
