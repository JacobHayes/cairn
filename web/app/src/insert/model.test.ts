import { describe, expect, it } from "vitest";

import type { Graph, GraphNode } from "../authoring/graph.ts";
import { NO_CHOICES, insertionMembers, planInsertion, type Choices } from "./model.ts";

const node = (key: string, extra: Partial<GraphNode> = {}): GraphNode => ({ key, id: key.replace("n_", ""), kind: "decision", title: key, ...extra });

/** The route: a group holding a decision that fills `reviewer`, and a deliverable that reads it when `reads` says so. */
function route(reads: boolean): Graph {
  return {
    roles: [{ key: "r_reviewer", id: "reviewer", title: "Reviewer" }],
    nodes: [
      node("n_review", { kind: "group", id: "review" }),
      node("n_who", { parent: "n_review", fills_role: "r_reviewer" }),
      node("n_model", { kind: "deliverable", parent: "n_review", ...(reads ? { relevant_when: { answered: "n_who" } } : {}) }),
    ],
  };
}

function plan(host: Graph, route: Graph, choices: Partial<Choices> = {}) {
  return planInsertion(host, route, { route: "security-review", version: 1 }, "i_one", { ...NO_CHOICES, ...choices });
}

describe("what the stepper sends", () => {
  it("wires the root both ways", () => {
    const host = { nodes: [node("n_phase", { kind: "group" }), node("n_a", { parent: "n_phase", id: "review" }), node("n_b")] } as Graph;
    const { mutation } = plan(host, route(false), { parent: "n_phase", after: ["n_a"], before: ["n_b"] });
    expect(mutation.edges).toEqual([
      { node: { route: "n_review" }, requires: { host: "n_a" } },
      { node: { host: "n_b" }, requires: { route: "n_review" } },
    ]);
  });

  // C19: a role the graph already fills leaves out the route's filling decision, unless a condition reads it.
  it.each([
    { name: "left out when the graph fills the role", reads: false, choices: {}, omit: ["n_who"], mapping: { existing: "r_reviewer" } },
    { name: "kept when a condition reads it: the role is new", reads: true, choices: {}, omit: [], mapping: "add" },
    { name: "kept when asked separately", reads: false, choices: { roles: { r_reviewer: "add" as const } }, omit: [], mapping: "add" },
  ])("$name", ({ reads, choices, omit, mapping }) => {
    const host = { roles: [{ key: "r_reviewer", id: "reviewer" }], nodes: [node("n_whois", { fills_role: "r_reviewer" })] } as Graph;
    const { mutation } = plan(host, route(reads), choices);
    expect(mutation.omit).toEqual(omit);
    expect(mutation.roles).toEqual({ r_reviewer: mapping });
  });

  it("offers only roles of the same cardinality", () => {
    const host = { roles: [{ key: "r_panel", id: "reviewer", multi: true }] } as Graph;
    const [row] = plan(host, route(false)).rows;
    expect(row).toMatchObject({ mapping: "add", options: [] });
  });
});

describe("which added nodes came from a route", () => {
  const pointer = { route: "security-review", version: 1 };
  const before = { nodes: [node("n_old", { from_route: pointer })] } as Graph;
  const after = { nodes: [node("n_old", { from_route: pointer }), node("n_review", { from_route: pointer }), node("n_threat", { parent: "n_review" })] } as Graph;

  it("names the route and version of each added node that carries the pointer, and no others", () => {
    const members = insertionMembers(before, after, () => "Security review");
    expect(Object.keys(members)).toEqual(["n_review"]);
    expect(members.n_review).toContain("Security review");
  });
});

describe("a route with several roots", () => {
  it("wires every root and leaves the root's name and id to the route", () => {
    const route = { nodes: [node("n_one", { id: "one" }), node("n_two", { id: "two" })] } as Graph;
    const { mutation, root } = plan({ nodes: [node("n_a")] }, route, { after: ["n_a"], title: "Renamed" });
    expect(mutation.edges).toHaveLength(2);
    expect(mutation.root_title).toBeUndefined();
    expect(root).toBeUndefined();
  });

  it("wires only the roots that are placed when a root is left out", () => {
    const routeGraph = { roles: [{ key: "r_reviewer", id: "reviewer" }], nodes: [node("n_who", { fills_role: "r_reviewer", id: "who" }), node("n_keep", { id: "keep", kind: "action" })] } as Graph;
    const host = { roles: [{ key: "r_reviewer", id: "reviewer" }], nodes: [node("n_a"), node("n_whois", { fills_role: "r_reviewer" })] } as Graph;
    const { mutation, root } = plan(host, routeGraph, { after: ["n_a"] });
    expect(mutation.omit).toEqual(["n_who"]);
    expect(mutation.edges).toEqual([{ node: { route: "n_keep" }, requires: { host: "n_a" } }]);
    expect(root).toEqual({ title: "n_keep" });
  });
});
