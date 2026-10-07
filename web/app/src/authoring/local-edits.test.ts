// B4 on the vendor evaluation journey, against the engine: an edit to a route-copied field
// sets its marker and a reset to the route writes the route's value back and clears it;
// removing route-copied nodes leaves tombstones, and restoring one adds a local copy of what
// was removed under fresh keys, at the path it had.
import type { Schema } from "@cairn/client";
import type { InBrowserHost } from "@cairn/wasm";
import { beforeAll, describe, expect, it } from "vitest";

import { planRemoval, removalMutations } from "./cascade.ts";
import { journeyAfter, seeded } from "./engine.test-support.ts";
import { pathOf, treeOf, type Graph, type GraphNode } from "./graph.ts";
import { resetMutations, restoreMutations, tombstonesOf, type LocalEdit } from "./local-edits.ts";

const JOURNEY = "j_vendor_eval";

const graphOf = (text: string): Graph => (JSON.parse(text) as Schema<"DomainDocument">).journey.graph;
const nodeIn = (graph: Graph, key: string): GraphNode | undefined => (graph.nodes ?? []).find((node) => node.key === key);
const markers = (graph: Graph, key: string): LocalEdit[] => graph.state?.local_edits?.[key] ?? [];

describe("reset to route (B4)", () => {
  let host: InBrowserHost;
  let version: Graph;
  beforeAll(async () => {
    host = await seeded();
    version = host.routeVersion("vendor-evaluation", 1).graph;
  });

  const edits: [string, Schema<"Mutation">, LocalEdit][] = [
    ["a title", { op: "set_node_field", node: "n_access", value: { title: "Sandbox access" } }, { field: "title" }],
    ["an estimate", { op: "set_node_field", node: "n_access", value: { estimate: 5 } }, { field: "estimate" }],
    ["a requirement", { op: "remove_edge", edge: { node: "n_plan", requires: "n_access" } }, { requires: "n_access" }],
    ["a resource", { op: "remove_resource", node: "n_access", resource: "a_access_request" }, { resource: "a_access_request" }],
  ];

  it.each(edits)("%s edited here is marked, and a reset puts the route's back and clears the marker", (_, edit, marker) => {
    const node = "node" in edit && typeof edit.node === "string" ? edit.node : "n_plan";
    const edited = journeyAfter(host, JOURNEY, [edit]);
    expect(markers(graphOf(edited), node)).toContainEqual(marker);
    const held = nodeIn(graphOf(edited), node);
    const reset = held === undefined ? undefined : resetMutations(held, nodeIn(version, node), marker);
    expect(reset).toBeDefined();
    const after = graphOf(journeyAfter(host, JOURNEY, reset ?? [], edited));
    expect(markers(after, node)).not.toContainEqual(marker);
    const route = nodeIn(version, node);
    const back = nodeIn(after, node);
    expect([back?.title, back?.estimate, back?.requires, back?.resources]).toEqual([route?.title, route?.estimate, route?.requires, route?.resources]);
  });

  it("offers no reset where the route has no such node to reset to", () => {
    const local: GraphNode = { key: "n_mine", id: "mine", kind: "action", title: "Mine" };
    expect(resetMutations(local, undefined, { field: "title" })).toBeUndefined();
  });
});

describe("tombstones and restore (B4)", () => {
  let host: InBrowserHost;
  let version: Graph;
  let removed: string;
  beforeAll(async () => {
    host = await seeded();
    version = host.routeVersion("vendor-evaluation", 1).graph;
    const plan = planRemoval(treeOf(graphOf(host.documentText(JOURNEY))), "n_partner_led");
    removed = journeyAfter(host, JOURNEY, plan === undefined ? [] : removalMutations(plan));
  });

  it("lists what was removed by its top, with how much went beneath it", () => {
    expect(tombstonesOf(graphOf(removed), version).map((stone) => [stone.node.key, stone.path, stone.beneath, stone.occupied])).toEqual([
      ["n_partner_led", "testing/partner-led", 2, false],
    ]);
  });

  it("restores a local copy of the subtree under fresh keys at the same path, which then holds its place", () => {
    const restore = restoreMutations(graphOf(removed), version, "n_partner_led");
    const keys = restore.flatMap((mutation) => (mutation.op === "add_node" ? [mutation.node.key] : []));
    expect(keys).toHaveLength(3);
    expect(keys.some((key) => ["n_partner_led", "n_criteria", "n_partner_results"].includes(key))).toBe(false);
    const after = graphOf(journeyAfter(host, JOURNEY, restore, removed));
    const tree = treeOf(after);
    expect(keys.map((key) => pathOf(tree, key))).toEqual(["testing/partner-led", "testing/partner-led/criteria", "testing/partner-led/partner-results"]);
    expect(keys.map((key) => after.state?.nodes?.[key]?.provenance)).toEqual(["local", "local", "local"]);
    expect(tombstonesOf(after, version).map((stone) => stone.occupied)).toEqual([true]);
  });
});
