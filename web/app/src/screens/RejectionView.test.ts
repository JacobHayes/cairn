// A stale refusal lists what changed only in words a person has: nodes and their fields.
import { describe, expect, it } from "vitest";

import { describeRecord } from "./RejectionView.tsx";

const title = (_journey: string, node: string) => (node === "n_a" ? "Kickoff" : "an item that was removed");
const graph = { journey: "j_one" };

describe("describeRecord", () => {
  it("names a node and a node's field by title and plain field name, never a key", () => {
    expect(describeRecord({ in_graph: { graph, key: { node: "n_a" } } }, title)).toBe('"Kickoff"');
    const words = describeRecord({ in_graph: { graph, key: { node_field: { node: "n_a", field: "due_by" } } } } as never, title);
    expect(words).toContain("Kickoff");
    expect(words).not.toContain("due_by");
  });

  it("names a node's answer or state by the node's title", () => {
    expect(describeRecord({ in_graph: { graph, key: { answer: "n_a" } } }, title)).toContain("Kickoff");
    expect(describeRecord({ in_graph: { graph, key: { node_state: { node: "n_a" } } } } as never, title)).toContain("Kickoff");
  });

  it("has no words for a record that is neither", () => {
    expect(describeRecord({ in_graph: { graph, key: { role: "r_x" } } }, title)).toBeUndefined();
    expect(describeRecord({ deployment: {} } as never, title)).toBeUndefined();
    expect(describeRecord({ in_graph: { graph: { route: "r_x" }, key: { node: "n_a" } } } as never, title)).toBeUndefined();
  });
});
