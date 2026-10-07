// C15: the layout is deterministic, reads left to right, holds each container's children
// inside it, and with the previous positions as hints a small edit moves few nodes. ELK runs
// in-thread here, as the worker runs it.
import ELK from "elkjs/lib/elk.bundled.js";
import { describe, expect, test } from "vitest";

import { LAYOUT_MOVED_FRACTION_MAX, hintsOf, layOut, movedNodes, type LayoutNode, type LayoutRequest, type Placement } from "./layout.ts";

const CARD = { width: 240, height: 96 };
const HEADER = 64;

/**
 * The vendor evaluation's whole canvas at the end of its scenario (fixtures/README.md), as
 * its level answers it with every kind shown: each node under its container, in tree order,
 * and the drawn edges.
 */
const TREE: [string, string?][] = [
  ["n_decision_meeting"],
  ["n_kickoff"],
  ["n_meeting_date"],
  ["n_partner_runs"],
  ["n_purpose"],
  ["n_reporting"],
  ["n_final_review", "n_reporting"],
  ["n_final_report", "n_final_review"],
  ["n_findings", "n_reporting"],
  ["n_findings_reviewer", "n_reporting"],
  ["n_review_opens", "n_reporting"],
  ["n_setup"],
  ["n_access", "n_setup"],
  ["n_plan", "n_setup"],
  ["n_plan_draft", "n_plan"],
  ["n_plan_review", "n_plan"],
  ["n_workload", "n_setup"],
  ["n_workload_ingest", "n_workload"],
  ["n_workload_query", "n_workload"],
  ["n_testing"],
  ["n_baseline", "n_testing"],
  ["n_comparison_set", "n_testing"],
  ["n_partner_led", "n_testing"],
  ["n_criteria", "n_partner_led"],
  ["n_partner_results", "n_partner_led"],
  ["n_who_informed"],
  ["n_who_owns"],
];
const EDGES: [string, string][] = [
  ["n_access", "n_plan"],
  ["n_comparison_set", "n_baseline"],
  ["n_criteria", "n_partner_results"],
  ["n_findings", "n_final_report"],
  ["n_findings", "n_findings_reviewer"],
  ["n_kickoff", "n_setup"],
  ["n_partner_runs", "n_partner_led"],
  ["n_plan", "n_comparison_set"],
  ["n_plan_draft", "n_plan_review"],
  ["n_review_opens", "n_final_review"],
  ["n_testing", "n_reporting"],
];

function request(tree: [string, string?][], edges: [string, string][]): LayoutRequest {
  const parents = new Set(tree.flatMap(([, parent]) => (parent === undefined ? [] : [parent])));
  const nodes: LayoutNode[] = tree.map(([key, parent]) => ({
    key,
    ...(parent === undefined ? {} : { parent }),
    ...CARD,
    ...(parents.has(key) ? { header: HEADER } : {}),
  }));
  return { nodes, edges: edges.map(([from, to]) => ({ from, to })) };
}

const fixture = request(TREE, EDGES);
const elk = () => new ELK();

/** Inserts `key` under `parent` after `after` in tree order. */
function inserted(key: string, after: string, parent: string): [string, string?][] {
  const at = TREE.findIndex(([each]) => each === after) + 1;
  return [...TREE.slice(0, at), [key, parent], ...TREE.slice(at)];
}

/** Every node's position on the canvas, adding each container's. */
function absolute(request: LayoutRequest, placed: Placement): Record<string, { x: number; y: number }> {
  const out: Record<string, { x: number; y: number }> = {};
  for (const node of request.nodes) {
    const own = placed[node.key] ?? { x: 0, y: 0 };
    const base = node.parent === undefined ? { x: 0, y: 0 } : (out[node.parent] ?? { x: 0, y: 0 });
    out[node.key] = { x: base.x + own.x, y: base.y + own.y };
  }
  return out;
}

describe("the layout (C15)", () => {
  test("the same graph lays out identically twice", async () => {
    const [once, twice] = [await layOut(elk(), fixture), await layOut(elk(), fixture)];
    expect(Object.keys(once)).toHaveLength(TREE.length);
    expect(twice).toEqual(once);
  });

  test("an edge between siblings runs left to right", async () => {
    const placed = await layOut(elk(), fixture);
    const parentOf = new Map(TREE.map(([key, parent]) => [key, parent]));
    const siblings = EDGES.filter(([from, to]) => parentOf.get(from) === parentOf.get(to));
    expect(siblings.length).toBeGreaterThan(0);
    for (const [from, to] of siblings) {
      expect(placed[from]?.x, `${from} -> ${to}`).toBeLessThan(placed[to]?.x ?? 0);
    }
  });

  test("a container holds its children below its header", async () => {
    const placed = await layOut(elk(), fixture);
    for (const [key, parent] of TREE.filter(([, each]) => each !== undefined)) {
      const [child, box] = [placed[key], placed[parent ?? ""]];
      expect(child && box, key).toBeTruthy();
      if (child === undefined || box === undefined) {
        continue;
      }
      expect(child.y, key).toBeGreaterThanOrEqual(HEADER);
      expect(child.x + child.width, key).toBeLessThanOrEqual(box.width);
      expect(child.y + child.height, key).toBeLessThanOrEqual(box.height);
    }
  });

  test("a revision laid out with its own positions as hints stays put", async () => {
    const placed = await layOut(elk(), fixture);
    expect(await layOut(elk(), { ...fixture, hints: hintsOf(placed) })).toEqual(placed);
  });

  const edits: [string, LayoutRequest][] = [
    ["a step added in a stage", request(inserted("n_new", "n_workload_query", "n_setup"), EDGES)],
    ["a step added after the plan", request(inserted("n_new", "n_workload_query", "n_setup"), [...EDGES, ["n_plan", "n_new"]])],
    ["a step added after the findings", request(inserted("n_new", "n_review_opens", "n_reporting"), [...EDGES, ["n_findings", "n_new"]])],
    ["a top-level milestone after the meeting", request([...TREE, ["n_new"]], [...EDGES, ["n_decision_meeting", "n_new"]])],
    ["a requirement added", request(TREE, [...EDGES, ["n_purpose", "n_setup"]])],
  ];
  test.each(edits)("%s moves fewer than the stated fraction of nodes", async (_, edited) => {
    const before = await layOut(elk(), fixture);
    const after = await layOut(elk(), { ...edited, hints: hintsOf(before) });
    const moved = movedNodes(before, after);
    expect(moved.length / TREE.length, moved.join(" ")).toBeLessThan(LAYOUT_MOVED_FRACTION_MAX);
  });

  test("a container's contents move with it and count once", async () => {
    const before = await layOut(elk(), fixture);
    const shifted: Placement = { ...before };
    const setup = before["n_setup"];
    expect(setup).toBeDefined();
    if (setup !== undefined) {
      shifted["n_setup"] = { ...setup, y: setup.y + 100 };
    }
    expect(movedNodes(before, shifted)).toEqual(["n_setup"]);
    const [was, now] = [absolute(fixture, before), absolute(fixture, shifted)];
    expect((now["n_plan_draft"]?.y ?? 0) - (was["n_plan_draft"]?.y ?? 0)).toBe(100);
  });
});
