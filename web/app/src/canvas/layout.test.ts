// C15: the layout is deterministic, reads left to right, holds each container's children
// inside it, and with the previous positions as hints a small edit moves few nodes. Every edge
// is routed in the gap the layout left for it, so its marker is never under a card (5.6), and
// the whole graph fits the viewport at the PRD limit (C3). ELK runs in-thread here, as the
// worker runs it.
import ELK from "elkjs/lib/elk.bundled.js";
import { describe, expect, test } from "vitest";

import { MARKER_LENGTH_PX } from "./EdgeLine.tsx";
import { fitZoom, minZoomOf } from "./gestures.ts";
import { LAYOUT_MOVED_FRACTION_MAX, edgeId, hintsOf, layOut, movedNodes, type LayoutNode, type LayoutRequest, type Placement } from "./layout.ts";

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
    expect(Object.keys(once.nodes)).toHaveLength(TREE.length);
    expect(twice).toEqual(once);
  });

  test("an edge between siblings runs left to right", async () => {
    const { nodes: placed } = await layOut(elk(), fixture);
    const parentOf = new Map(TREE.map(([key, parent]) => [key, parent]));
    const siblings = EDGES.filter(([from, to]) => parentOf.get(from) === parentOf.get(to));
    expect(siblings.length).toBeGreaterThan(0);
    for (const [from, to] of siblings) {
      expect(placed[from]?.x, `${from} -> ${to}`).toBeLessThan(placed[to]?.x ?? 0);
    }
  });

  test("a container holds its children below its header", async () => {
    const { nodes: placed } = await layOut(elk(), fixture);
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
    const moved = movedNodes(before.nodes, after.nodes);
    expect(moved.length / TREE.length, moved.join(" ")).toBeLessThan(LAYOUT_MOVED_FRACTION_MAX);
  });

  test("a container's contents move with it and count once", async () => {
    const { nodes: before } = await layOut(elk(), fixture);
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

describe("edges and the whole graph", () => {
  test("every edge is routed with room for its marker: the stretch before its tip is long enough and clear of every card (5.6)", async () => {
    const laid = await layOut(elk(), fixture);
    const where = absolute(fixture, laid.nodes);
    const parents = new Set(TREE.flatMap(([, parent]) => (parent === undefined ? [] : [parent])));
    for (const [from, to] of EDGES) {
      const [tip, before] = (laid.routes[edgeId({ from, to })] ?? []).slice(-2).reverse();
      expect(tip && before, `${from} -> ${to} has a route`).toBeTruthy();
      const length = Math.hypot((tip?.x ?? 0) - (before?.x ?? 0), (tip?.y ?? 0) - (before?.y ?? 0));
      expect(length, `${from} -> ${to}: room for its marker`).toBeGreaterThanOrEqual(MARKER_LENGTH_PX);
      // The stretch runs into a card's west side: where the marker begins must be in the gap, not under another card.
      const back = { x: (tip?.x ?? 0) - ((tip?.x ?? 0) - (before?.x ?? 0)) * (MARKER_LENGTH_PX / length), y: (tip?.y ?? 0) - ((tip?.y ?? 0) - (before?.y ?? 0)) * (MARKER_LENGTH_PX / length) };
      const under = TREE.filter(([key]) => !parents.has(key)).filter(([key]) => {
        const [at, size] = [where[key], laid.nodes[key]];
        return at !== undefined && size !== undefined && back.x > at.x + 0.5 && back.x < at.x + size.width - 0.5 && back.y > at.y + 0.5 && back.y < at.y + size.height - 0.5;
      });
      expect(under, `${from} -> ${to}: its marker begins under a card`).toEqual([]);
    }
  });

  test("fit-all on 2,000 generated nodes keeps every card inside the viewport (C3); the layout time is reported", async () => {
    const [layers, perLayer] = [40, 50];
    const key = (layer: number, at: number) => `n_${String(layer)}_${String(at % perLayer)}`;
    const nodes: LayoutNode[] = [];
    const edges: { from: string; to: string }[] = [];
    for (let layer = 0; layer < layers; layer += 1) {
      for (let at = 0; at < perLayer; at += 1) {
        nodes.push({ key: key(layer, at), width: 240, height: 90 });
        if (layer > 0) {
          edges.push({ from: key(layer - 1, at), to: key(layer, at) }, { from: key(layer - 1, at + 7), to: key(layer, at) });
        }
      }
    }
    const started = performance.now();
    const laid = await layOut(elk(), { nodes, edges });
    console.info(`layout of ${String(nodes.length)} nodes and ${String(edges.length)} edges: ${String(Math.round(performance.now() - started))} ms`);
    const boxes = Object.values(laid.nodes);
    const bounds = { width: Math.max(...boxes.map((box) => box.x + box.width)), height: Math.max(...boxes.map((box) => box.y + box.height)) };
    const fit = fitZoom(bounds, { width: 840, height: 650 }, { top: 24, right: 24, bottom: 24, left: 24 });
    // The canvas can zoom out as far as the whole graph needs (a fixed 0.1 could not: the fit is below it).
    expect(fit).toBeLessThan(0.1);
    expect(minZoomOf(fit)).toBeLessThanOrEqual(fit);
  }, 60_000);
});
