// C15: the canvas's automatic layout, ELK's layered algorithm over one level of the canvas
// (ARCHITECTURE, Web UI: Canvas). Deterministic: the same request lays out identically, and
// a request carries its nodes in tree order and its edges sorted, so the same level always
// makes the same request. Small edits move few nodes: a request may carry the previous
// revision's positions as hints, which ELK's semi-interactive crossing minimization keeps the
// order of nodes within each layer by
// (decisions/2026-10-07-the-layout-is-hinted-by-the-views-last-positions-a-fresh.md). ELK
// runs in a worker (layout-worker.ts); this module is the pure part both sides share, and
// what the unit tests run in-thread.
//
// Cost at `node_count_max` (2,000 nodes, at most 64 explicit edges each): building the ELK
// graph and reading its positions are O(nodes + edges); ELK's layered algorithm is roughly
// O((nodes + edges) x layers) with its crossing-minimization sweeps bounded by its own
// iteration limits, which is why it runs in a worker and never blocks input.
import type { ElkExtendedEdge, ElkNode, LayoutOptions } from "elkjs/lib/elk-api.js";

/** One node to place: a card's size, or none for a container ELK sizes around its children. */
export interface LayoutNode {
  key: string;
  /** The container it is drawn in; none at the top level. */
  parent?: string;
  width: number;
  height: number;
  /** Space above a container's children for its own header. */
  header?: number;
}

/** A point, relative to the node's container. */
export interface Point {
  x: number;
  y: number;
}

/** What one layout places: the nodes in tree order (parents first), the edges sorted. */
export interface LayoutRequest {
  nodes: LayoutNode[];
  edges: { from: string; to: string }[];
  /** The previous revision's positions of the same view, by key, relative to each container. */
  hints?: Record<string, Point>;
}

/** Where a node lands, relative to its container, and its size (a container's as ELK sized it). */
export interface Placed extends Point {
  width: number;
  height: number;
}

/** Each node's place, by key. */
export type Placement = Record<string, Placed>;

/**
 * C15's stated bound: adding one node (or one requirement) to the vendor evaluation's canvas,
 * laid out with the previous positions as hints, moves fewer than this fraction of its nodes
 * within their containers
 * (decisions/2026-10-07-the-layout-is-hinted-by-the-views-last-positions-a-fresh.md).
 */
export const LAYOUT_MOVED_FRACTION_MAX = 0.25;

/** The space between nodes, between layers, and inside a container (px). */
export const NODE_GAP_PX = 28;
export const LAYER_GAP_PX = 56;
export const CONTAINER_PAD_PX = 14;

/** C15: ELK's options for every layout; left to right, as the PRD's flowcharts read. */
const ROOT_OPTIONS: LayoutOptions = {
  "elk.algorithm": "layered",
  "elk.direction": "RIGHT",
  // Edges between levels of the tree are laid out with the whole graph (C3: one canvas).
  "elk.hierarchyHandling": "INCLUDE_CHILDREN",
  "elk.spacing.nodeNode": String(NODE_GAP_PX),
  "elk.layered.spacing.nodeNodeBetweenLayers": String(LAYER_GAP_PX),
  "elk.spacing.componentComponent": String(LAYER_GAP_PX),
  "elk.randomSeed": "1",
};

/** The ELK graph for a request. */
export function elkGraph(request: LayoutRequest): ElkNode {
  const hinted = request.hints !== undefined && Object.keys(request.hints).length > 0;
  const shapes = new Map<string, ElkNode>();
  const roots: ElkNode[] = [];
  const parents = new Set(request.nodes.flatMap((node) => (node.parent === undefined ? [] : [node.parent])));
  for (const node of request.nodes) {
    const shape: ElkNode = { id: node.key, layoutOptions: {} };
    if (parents.has(node.key)) {
      shape.children = [];
      shape.layoutOptions = {
        "elk.padding": `[top=${String(node.header ?? 0)},left=${String(CONTAINER_PAD_PX)},bottom=${String(CONTAINER_PAD_PX)},right=${String(CONTAINER_PAD_PX)}]`,
        // A container is never narrower than its own card.
        "elk.nodeSize.constraints": "MINIMUM_SIZE",
        "elk.nodeSize.minimum": `(${String(node.width)}, ${String(node.height)})`,
      };
    } else {
      shape.width = node.width;
      shape.height = node.height;
    }
    const hint = request.hints?.[node.key];
    if (hint !== undefined && shape.layoutOptions !== undefined) {
      shape.layoutOptions["elk.position"] = `(${String(hint.x)}, ${String(hint.y)})`;
    }
    shapes.set(node.key, shape);
    const container = node.parent === undefined ? undefined : shapes.get(node.parent);
    if (container === undefined) {
      roots.push(shape);
    } else {
      container.children?.push(shape);
    }
  }
  const edges: ElkExtendedEdge[] = request.edges
    .filter((edge) => shapes.has(edge.from) && shapes.has(edge.to))
    .map((edge, index) => ({ id: `e${String(index)}`, sources: [edge.from], targets: [edge.to] }));
  const layoutOptions: LayoutOptions = { ...ROOT_OPTIONS };
  if (hinted) {
    layoutOptions["elk.layered.crossingMinimization.semiInteractive"] = "true";
  }
  return { id: "root", layoutOptions, children: roots, edges };
}

/** Each placed node of a laid-out ELK graph, relative to its container. */
export function placement(graph: ElkNode): Placement {
  const placed: Placement = {};
  const walk = (node: ElkNode) => {
    for (const child of node.children ?? []) {
      placed[child.id] = { x: child.x ?? 0, y: child.y ?? 0, width: child.width ?? 0, height: child.height ?? 0 };
      walk(child);
    }
  };
  walk(graph);
  return placed;
}

/** What lays out an ELK graph: ELK itself, in a worker or in-thread. */
export interface Elk {
  layout(graph: ElkNode): Promise<ElkNode>;
}

/** C15: lays out a request with `elk`. */
export async function layOut(elk: Elk, request: LayoutRequest): Promise<Placement> {
  return placement(await elk.layout(elkGraph(request)));
}

/** A placement's positions, as hints for the next revision's layout of the same view. */
export function hintsOf(placed: Placement): Record<string, Point> {
  return Object.fromEntries(Object.entries(placed).map(([key, { x, y }]) => [key, { x, y }]));
}

/**
 * C15: the nodes of `before` that `after` moved within their container (by more than a
 * pixel), so a container that slid as a whole counts once and its contents do not. Nodes
 * either side lacks are not counted.
 */
export function movedNodes(before: Placement, after: Placement): string[] {
  return Object.keys(before)
    .filter((key) => {
      const [was, now] = [before[key], after[key]];
      return was !== undefined && now !== undefined && (Math.abs(was.x - now.x) > 1 || Math.abs(was.y - now.y) > 1);
    })
    .sort();
}

/** The text that identifies a request's input, so an unchanged view is not laid out again. */
export function signature(request: Omit<LayoutRequest, "hints">): string {
  return JSON.stringify([request.nodes, request.edges]);
}
