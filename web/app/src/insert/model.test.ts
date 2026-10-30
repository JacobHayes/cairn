import { describe, expect, it } from "vitest";

import type { Schema } from "@cairn/client";

import type { Graph, GraphNode } from "../authoring/graph.ts";
import { NO_CHOICES, insertionMembers, planInsertion, type Choices } from "./model.ts";

const node = (key: string, extra: Partial<GraphNode> = {}): GraphNode => ({ key, id: key.replace("n_", ""), kind: "decision", title: key, ...extra });

/** The engine's plan, as far as the wiring reads it. */
function plan(host: Graph, route: Graph, engine: Schema<"InsertionPlan">, choices: Partial<Choices> = {}) {
  return planInsertion(host, route, engine, { route: "security-review", version: 1 }, "i_one", { ...NO_CHOICES, ...choices });
}

describe("what the stepper sends", () => {
  it("wires the root both ways", () => {
    const route = { nodes: [node("n_review", { kind: "group", id: "review" })] } as Graph;
    const host = { nodes: [node("n_phase", { kind: "group" }), node("n_a", { parent: "n_phase", id: "review" }), node("n_b")] } as Graph;
    const { mutation } = plan(host, route, { roots: ["n_review"] }, { parent: "n_phase", after: ["n_a"], before: ["n_b"] });
    expect(mutation.edges).toEqual([
      { node: { route: "n_review" }, requires: { host: "n_a" } },
      { node: { host: "n_b" }, requires: { route: "n_review" } },
    ]);
  });

  it("sends the engine's mapping of every role and what it leaves out, and names the graph's roles in the rows", () => {
    const route = { roles: [{ key: "r_reviewer", id: "reviewer", title: "Reviewer" }], nodes: [node("n_who", { fills_role: "r_reviewer" }), node("n_keep", { kind: "action" })] } as Graph;
    const host = { roles: [{ key: "r_lead", id: "reviewer", title: "Lead reviewer" }], nodes: [node("n_whois", { title: "Who reviews", fills_role: "r_lead" })] } as Graph;
    const engine: Schema<"InsertionPlan"> = {
      roles: [{ key: "r_reviewer", existing: "r_lead", options: ["r_lead"], filled: { by: "n_whois", left_out: ["n_who"] } }],
      omit: ["n_who"],
      roots: ["n_keep"],
    };
    const { mutation, rows, root } = plan(host, route, engine);
    expect(mutation).toMatchObject({ roles: { r_reviewer: { existing: "r_lead" } }, kinds: {}, omit: ["n_who"] });
    expect(rows).toMatchObject([{ type: "role", title: "Reviewer", mapping: { existing: "r_lead" }, options: [{ key: "r_lead", title: "Lead reviewer" }], filled: { by: "Who reviews", leftOut: [{ key: "n_who" }] } }]);
    expect(root).toEqual({ title: "n_keep" });
  });

  it("maps a role the engine lands on a new one to add", () => {
    const route = { roles: [{ key: "r_reviewer", id: "reviewer" }] } as Graph;
    const { mutation } = plan({}, route, { roles: [{ key: "r_reviewer" }], roots: [] });
    expect(mutation.roles).toEqual({ r_reviewer: "add" });
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
  it("wires every root and leaves the root's name to the route", () => {
    const route = { nodes: [node("n_one", { id: "one" }), node("n_two", { id: "two" })] } as Graph;
    const { mutation, root } = plan({ nodes: [node("n_a")] }, route, { roots: ["n_one", "n_two"] }, { after: ["n_a"], title: "Renamed" });
    expect(mutation.edges).toHaveLength(2);
    expect(mutation.root_title).toBeUndefined();
    expect(root).toBeUndefined();
  });
});
