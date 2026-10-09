import { describe, expect, it } from "vitest";

import { treeOf, type Graph, type GraphNode } from "../authoring/graph.ts";
import { NO_CHOICES, planInsertion, type Choices } from "./model.ts";

const node = (key: string, extra: Partial<GraphNode> = {}): GraphNode => ({ key, id: key.replace("n_", ""), kind: "decision", title: key, ...extra });

/** The segment: a group holding a decision that fills `reviewer`, and a deliverable that reads it when `reads` says so. */
function segment(reads: boolean): Graph {
  return {
    roles: [{ key: "r_reviewer", id: "reviewer", title: "Reviewer" }],
    nodes: [
      node("n_review", { kind: "group", id: "review" }),
      node("n_who", { parent: "n_review", fills_role: "r_reviewer" }),
      node("n_model", { kind: "deliverable", parent: "n_review", ...(reads ? { relevant_when: { answered: "n_who" } } : {}) }),
    ],
  };
}

function plan(host: Graph, seg: Graph, choices: Partial<Choices> = {}) {
  return planInsertion(host, treeOf(host), seg, { route: "security-review", version: 1 }, "i_one", { ...NO_CHOICES, ...choices });
}

describe("what the stepper sends", () => {
  it("wires the root both ways and takes the first free id among its siblings", () => {
    const host = { nodes: [node("n_phase", { kind: "group" }), node("n_a", { parent: "n_phase", id: "review" }), node("n_b")] } as Graph;
    const { mutation, root } = plan(host, segment(false), { parent: "n_phase", after: ["n_a"], before: ["n_b"] });
    expect(mutation.edges).toEqual([
      { node: { segment: "n_review" }, requires: { host: "n_a" } },
      { node: { host: "n_b" }, requires: { segment: "n_review" } },
    ]);
    expect(root).toMatchObject({ id: "review-2", idTaken: true });
  });

  // C19: a role the graph already fills leaves out the segment's filling decision, unless a condition reads it.
  it.each([
    { name: "left out when the graph fills the role", reads: false, choices: {}, omit: ["n_who"], mapping: { existing: "r_reviewer" } },
    { name: "kept when a condition reads it: the role is new", reads: true, choices: {}, omit: [], mapping: "add" },
    { name: "kept when asked separately", reads: false, choices: { roles: { r_reviewer: "add" as const } }, omit: [], mapping: "add" },
  ])("$name", ({ reads, choices, omit, mapping }) => {
    const host = { roles: [{ key: "r_reviewer", id: "reviewer" }], nodes: [node("n_whois", { fills_role: "r_reviewer" })] } as Graph;
    const { mutation } = plan(host, segment(reads), choices);
    expect(mutation.omit).toEqual(omit);
    expect(mutation.roles).toEqual({ r_reviewer: mapping });
  });

  it("offers only roles of the same cardinality", () => {
    const host = { roles: [{ key: "r_panel", id: "reviewer", multi: true }] } as Graph;
    const [row] = plan(host, segment(false)).rows;
    expect(row).toMatchObject({ mapping: "add", options: [] });
  });
});
