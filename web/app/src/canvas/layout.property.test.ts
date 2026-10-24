// C15: over generated graphs up to the limits, the layout never lets two siblings overlap, keeps
// every child wholly inside its container and below the container's header, and never lets a
// group overlap its sibling groups. ELK runs in-thread, as in layout.test.ts. The cases come from
// a seeded generator, so a run is the same every time; every case must pass, and a failure prints
// the request that broke it.
import { CONTAINMENT_DEPTH_MAX, EDGE_COUNT_PER_NODE_MAX, NODE_COUNT_MAX } from "@cairn/client";
import { describe, test } from "vitest";

import { CARD_WIDTH_PX } from "./cards.ts";
import { type LayoutNode, type LayoutRequest } from "./layout.ts";
import { expectGeometry } from "./layout.test-support.ts";

/** Cards are one to three rows of text tall, a container's header taller with its progress and marker rows. */
const CARD_HEIGHT_PX = { min: 64, max: 160 };
const HEADER_PX = { min: 64, max: 200 };

/** A small seeded generator (mulberry32): the same seed always draws the same numbers. */
function generator(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

interface Shape {
  nodeCount: number;
  depthMax: number;
  edgeCount: number;
  /** The share of nodes that become containers (some of which stay empty of children). */
  groupShare: number;
}

/** A random tree in tree order (parents first) with edges, within the limits. */
function generated(seed: number, shape: Shape): LayoutRequest {
  if (shape.nodeCount > NODE_COUNT_MAX || shape.depthMax > CONTAINMENT_DEPTH_MAX) {
    throw new Error("the shape is past the limits");
  }
  const next = generator(seed);
  const between = (min: number, max: number) => min + Math.floor(next() * (max - min + 1));
  const parents: (number | undefined)[] = [];
  const depths: number[] = [];
  const open: number[] = [];
  for (let at = 0; at < shape.nodeCount; at += 1) {
    const candidates = open.filter((each) => (depths[each] ?? 0) + 1 < shape.depthMax);
    const parent = candidates.length > 0 && next() < 0.8 ? candidates[between(0, candidates.length - 1)] : undefined;
    parents.push(parent);
    depths.push(parent === undefined ? 0 : (depths[parent] ?? 0) + 1);
    if (next() < shape.groupShare) {
      open.push(at);
    }
  }
  const containers = new Set(parents.flatMap((parent) => (parent === undefined ? [] : [parent])));
  const nodes: LayoutNode[] = parents.map((parent, at) => {
    const height = containers.has(at) ? between(HEADER_PX.min, HEADER_PX.max) : between(CARD_HEIGHT_PX.min, CARD_HEIGHT_PX.max);
    return { key: `n_${String(at)}`, ...(parent === undefined ? {} : { parent: `n_${String(parent)}` }), width: CARD_WIDTH_PX, height, ...(containers.has(at) ? { header: height } : {}) };
  });
  const degrees = new Map<number, number>();
  const edges = new Map<string, { from: string; to: string }>();
  for (let tries = 0; edges.size < shape.edgeCount && tries < shape.edgeCount * 4; tries += 1) {
    const [from, to] = [between(0, shape.nodeCount - 1), between(0, shape.nodeCount - 1)];
    const [fromDegree, toDegree] = [degrees.get(from) ?? 0, degrees.get(to) ?? 0];
    // Edges run between nodes that neither contain the other, and stay within the per-node limit.
    if (from >= to || fromDegree >= EDGE_COUNT_PER_NODE_MAX || toDegree >= EDGE_COUNT_PER_NODE_MAX || ancestors(parents, to).has(from)) {
      continue;
    }
    degrees.set(from, fromDegree + 1);
    degrees.set(to, toDegree + 1);
    edges.set(`${String(from)}>${String(to)}`, { from: `n_${String(from)}`, to: `n_${String(to)}` });
  }
  const sorted = [...edges.values()].sort((a, b) => (a.from + a.to < b.from + b.to ? -1 : 1));
  return { nodes, edges: sorted };
}

function ancestors(parents: (number | undefined)[], at: number): Set<number> {
  const found = new Set<number>();
  for (let each = parents[at]; each !== undefined; each = parents[each]) {
    found.add(each);
  }
  return found;
}

describe("the layout holds its geometry over generated graphs (C15)", () => {
  test("random graphs from a handful of nodes to a hundred and fifty", async () => {
    const shapes: [Shape, number][] = [
      [{ nodeCount: 6, depthMax: 3, edgeCount: 4, groupShare: 0.4 }, 20],
      [{ nodeCount: 25, depthMax: 4, edgeCount: 25, groupShare: 0.3 }, 20],
      [{ nodeCount: 60, depthMax: CONTAINMENT_DEPTH_MAX, edgeCount: 90, groupShare: 0.35 }, 10],
      [{ nodeCount: 150, depthMax: 8, edgeCount: 200, groupShare: 0.2 }, 3],
    ];
    for (const [shape, seeds] of shapes) {
      for (let seed = 1; seed <= seeds; seed += 1) {
        await expectGeometry(`seed ${String(seed)} of ${JSON.stringify(shape)}`, generated(seed, shape));
      }
    }
  }, 60_000);

  describe("edge cases", () => {
    const card = (key: string, parent?: string, height = 96): LayoutNode => ({ key, ...(parent === undefined ? {} : { parent }), width: CARD_WIDTH_PX, height });
    const group = (key: string, parent?: string, header = 100): LayoutNode => ({ ...card(key, parent, header), header });

    test("empty groups beside cards and each other, with edges to and from them", async () => {
      const nodes = [group("g1"), group("g2"), card("a"), card("b"), group("g3", "g2"), card("c", "g2")];
      // A group with no children is laid out as a card, never as a container.
      await expectGeometry("empty groups", { nodes, edges: [{ from: "g1", to: "g2" }, { from: "a", to: "g1" }, { from: "g3", to: "c" }] });
    });

    test("nesting to the depth limit", async () => {
      const nodes: LayoutNode[] = [];
      for (let depth = 0; depth < CONTAINMENT_DEPTH_MAX; depth += 1) {
        nodes.push(group(`g${String(depth)}`, depth === 0 ? undefined : `g${String(depth - 1)}`, 180), card(`c${String(depth)}`, `g${String(depth)}`, 160));
      }
      await expectGeometry("deep nesting", { nodes, edges: [{ from: "c0", to: "c5" }, { from: "c3", to: "c15" }] });
    });

    test("wide fan-out and fan-in at the per-node edge limit, across containers", async () => {
      const nodes = [card("hub"), group("g"), card("sink"), ...Array.from({ length: EDGE_COUNT_PER_NODE_MAX }, (_, at) => card(`t${String(at)}`, "g", 64 + at))];
      const edges = nodes.filter((node) => node.parent === "g").flatMap((node) => [{ from: "hub", to: node.key }, { from: node.key, to: "sink" }]);
      await expectGeometry("fan-out", { nodes, edges });
    });

    test("the tallest headers and cards beside the shortest", async () => {
      const nodes = [group("tall", undefined, 400), card("tall_child", "tall", 400), group("short", undefined, 64), card("short_child", "short", 64), card("free", undefined, 400)];
      await expectGeometry("extremes", { nodes, edges: [{ from: "short_child", to: "tall_child" }, { from: "free", to: "short" }] });
    });
  });
});
