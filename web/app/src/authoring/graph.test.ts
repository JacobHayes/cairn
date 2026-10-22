// A2: a node moves only into a container outside itself.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { moveTargets, treeOf } from "./graph.ts";
import { mintKey, slugOf, uniqueId } from "./keys.ts";

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

describe("ids and keys (Keys and references)", () => {
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
