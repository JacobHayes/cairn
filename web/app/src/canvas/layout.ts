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
  /** Orders the dates imply between sibling cards, which the layout reads as a preference (`orderEdgesOf`). */
  order?: { from: string; to: string }[];
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

/** Where each edge runs, by `edgeId`: points on the canvas (not relative to a container) from its source to its target. */
export type Routes = Record<string, Point[]>;

/** What one layout answers: the nodes' places and the edges' routes. */
export interface Layout {
  nodes: Placement;
  routes: Routes;
}

/**
 * C15's stated bound: adding one node (or one requirement) to the vendor evaluation's canvas,
 * laid out with the previous positions as hints, moves fewer than this fraction of its nodes
 * within their containers
 * (decisions/2026-10-07-the-layout-is-hinted-by-the-views-last-positions-a-fresh.md).
 */
export const LAYOUT_MOVED_FRACTION_MAX = 0.25;

/** The space between nodes, between layers, and inside a container (px). */
export const NODE_GAP_PX = 24;
export const LAYER_GAP_PX = 96;
export const CONTAINER_PAD_PX = 20;

/** The spacing a container's children need, the same as the top level's so arrows keep room to bend. */
const INNER_SPACING: LayoutOptions = {
  "elk.spacing.nodeNode": String(NODE_GAP_PX),
  "elk.layered.spacing.nodeNodeBetweenLayers": String(LAYER_GAP_PX),
  "elk.layered.spacing.edgeNodeBetweenLayers": "24",
  "elk.layered.spacing.edgeEdgeBetweenLayers": "12",
  "elk.spacing.edgeNode": "16",
  "elk.spacing.edgeEdge": "10",
};

/** C15: ELK's options for every layout; left to right, as the PRD's flowcharts read. */
const ROOT_OPTIONS: LayoutOptions = {
  "elk.algorithm": "layered",
  "elk.direction": "RIGHT",
  // Edges between levels of the tree are laid out with the whole graph (C3: one canvas).
  "elk.hierarchyHandling": "INCLUDE_CHILDREN",
  ...INNER_SPACING,
  "elk.spacing.componentComponent": String(LAYER_GAP_PX),
  // Edges are routed in the gaps INNER_SPACING leaves, so an arrowhead is never under a card (5.6).
  "elk.edgeRouting": "ORTHOGONAL",
  // Brandes-Koepf balanced, not network simplex: on the vendor evaluation, one added requirement
  // moved up to 12 of its 27 nodes with network simplex, past C15's bound; balanced keeps every
  // edit within it (layout.test.ts holds the five edits).
  "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
  "elk.layered.nodePlacement.bk.fixedAlignment": "BALANCED",
  // Route points come back in the root's coordinates whatever container the edge crosses.
  "org.eclipse.elk.json.edgeCoords": "ROOT",
  "elk.randomSeed": "1",
};

/** An edge's id in the layout and on the canvas: the ends it joins. */
export function edgeId(edge: { from: string; to: string }): string {
  return `${edge.from}->${edge.to}`;
}

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
        "elk.padding": `[top=${String((node.header ?? 0) + CONTAINER_PAD_PX)},left=${String(CONTAINER_PAD_PX)},bottom=${String(CONTAINER_PAD_PX)},right=${String(CONTAINER_PAD_PX)}]`,
        // A container is never narrower than its own card.
        "elk.nodeSize.constraints": "MINIMUM_SIZE",
        "elk.nodeSize.minimum": `(${String(node.width)}, ${String(node.height)})`,
        // With INCLUDE_CHILDREN ELK reads a nested graph's spacing from its own container, not the
        // root, so the children would fall back to ELK's defaults (about 20px between layers).
        ...INNER_SPACING,
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
    .map((edge) => ({ id: edgeId(edge), sources: [edge.from], targets: [edge.to] }));
  const ordered: ElkExtendedEdge[] = (request.order ?? [])
    .filter((pair) => shapes.has(pair.from) && shapes.has(pair.to))
    .map((pair) => ({ id: `order:${edgeId(pair)}`, sources: [pair.from], targets: [pair.to] }));
  const layoutOptions: LayoutOptions = { ...ROOT_OPTIONS };
  if (hinted) {
    layoutOptions["elk.layered.crossingMinimization.semiInteractive"] = "true";
  }
  return { id: "root", layoutOptions, children: roots, edges: [...edges, ...ordered] };
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

/** Each laid-out edge's route (5.6): ELK's orthogonal sections, start to bends to end, in root coordinates. */
export function routes(graph: ElkNode): Routes {
  const routed: Routes = {};
  for (const edge of graph.edges ?? []) {
    const points = (edge.sections ?? []).flatMap((section) => [section.startPoint, ...(section.bendPoints ?? []), section.endPoint]);
    if (points.length >= 2) {
      routed[edge.id] = points.map(({ x, y }) => ({ x, y }));
    }
  }
  return routed;
}

/** What lays out an ELK graph: ELK itself, in a worker or in-thread. */
export interface Elk {
  layout(graph: ElkNode): Promise<ElkNode>;
}

/** C15: lays out a request with `elk`. */
export async function layOut(elk: Elk, request: LayoutRequest): Promise<Layout> {
  const laid = await elk.layout(elkGraph(request));
  return { nodes: placement(laid), routes: routes(laid) };
}

/** A layout's positions, as hints for the next revision's layout of the same view. */
export function hintsOf(laid: Layout): Record<string, Point> {
  return Object.fromEntries(Object.entries(laid.nodes).map(([key, { x, y }]) => [key, { x, y }]));
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
  return JSON.stringify([request.nodes, request.edges, request.order ?? []]);
}

/**
 * C1, C15: the orders the dates imply that the layout may read. ELK's edge priorities are only
 * preferences (one dependency against a dozen timing paths reversed the dependency), so an order
 * that runs against the dependencies is removed before ELK sees it: every waits-on line and every
 * order is lifted to the two siblings under the lowest container holding both ends (the cards ELK
 * orders against each other); each container's children are ranked by the waits-on pairs alone
 * (Kahn's algorithm, preferring among those ready one with no unranked order before it, then the
 * lowest key); an order is kept when both lifted ends are ranked, the first ranks before the
 * second, and no waits-on line already joins them that way. What is kept runs forward in an order
 * the dependencies also run forward in, so the union is acyclic and ELK reverses nothing.
 */
export function orderEdgesOf(
  cards: readonly { key: string; parent?: string | undefined }[],
  lines: readonly { from: string; to: string }[],
  order: readonly { from: string; to: string }[],
): { from: string; to: string }[] {
  const parents = new Map(cards.map((card) => [card.key, card.parent]));
  const chain = (key: string) => {
    const above: string[] = [];
    for (let at: string | undefined = key; at !== undefined && above.length <= cards.length; at = parents.get(at)) {
      above.unshift(at);
    }
    return above;
  };
  const lift = (pair: { from: string; to: string }) => {
    const [up, down] = [chain(pair.from), chain(pair.to)];
    const at = up.findIndex((key, index) => key !== down[index]);
    const [from, to] = [up[at], down[at]];
    return at < 0 || from === undefined || to === undefined ? undefined : { from, to, container: at === 0 ? "" : (up[at - 1] ?? "") };
  };
  const required = lines.flatMap((line) => lift(line) ?? []);
  const wanted = order.flatMap((pair) => lift(pair) ?? []);
  const joined = new Set(required.map((pair) => `${pair.from}\u0000${pair.to}`));
  const containers = new Set([...required, ...wanted].map((pair) => pair.container));
  const kept = new Map<string, { from: string; to: string }>();
  for (const container of containers) {
    const [needs, hopes] = [required.filter((pair) => pair.container === container), wanted.filter((pair) => pair.container === container)];
    const members = [...new Set([...needs, ...hopes].flatMap((pair) => [pair.from, pair.to]))].sort();
    // How many of each member's predecessors are still unranked, by requirement and by date, and who each member precedes.
    const [needLeft, hopeLeft] = [new Map(members.map((key) => [key, 0])), new Map(members.map((key) => [key, 0]))];
    const [needNext, hopeNext] = [new Map<string, string[]>(), new Map<string, string[]>()];
    for (const [pairs, left, next] of [[needs, needLeft, needNext], [hopes, hopeLeft, hopeNext]] as const) {
      for (const pair of pairs) {
        left.set(pair.to, (left.get(pair.to) ?? 0) + 1);
        next.set(pair.from, [...(next.get(pair.from) ?? []), pair.to]);
      }
    }
    // The next card is one nothing it requires is left for, preferring one no date puts after an unranked card, then the lowest key.
    const rank = new Map<string, number>();
    while (rank.size < members.length) {
      const ready = members.filter((key) => !rank.has(key) && needLeft.get(key) === 0);
      const next = ready.find((key) => hopeLeft.get(key) === 0) ?? ready[0];
      if (next === undefined) {
        break;
      }
      rank.set(next, rank.size);
      for (const [nexts, left] of [[needNext, needLeft], [hopeNext, hopeLeft]] as const) {
        for (const key of nexts.get(next) ?? []) {
          left.set(key, (left.get(key) ?? 0) - 1);
        }
      }
    }
    for (const pair of hopes) {
      const [a, b] = [rank.get(pair.from), rank.get(pair.to)];
      if (a !== undefined && b !== undefined && a < b && !joined.has(`${pair.from}\u0000${pair.to}`)) {
        kept.set(`${pair.from}\u0000${pair.to}`, { from: pair.from, to: pair.to });
      }
    }
  }
  return [...kept.values()].sort((a, b) => edgeId(a).localeCompare(edgeId(b)));
}
