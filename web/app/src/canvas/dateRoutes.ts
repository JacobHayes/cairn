// C1: where the date lines of the selected node run. Lines leave and enter a card's sides at
// ports of their own, spaced evenly down the side, so no two arrowheads stack; and each is
// routed round the cards and the stage frames it has no business in, through the gutters
// between them, on a grid that the layout's spacing leaves room for. Pure.
import type { Box } from "./EdgeLine.tsx";
import type { Point } from "./layout.ts";

/** The space (px) kept between a route and a card it passes. */
const CLEARANCE_PX = 10;
/** The straight stretch (px) a route leaves a port by, so a bend never touches the card and an arrowhead has room. */
export const STUB_PX = 20;
/** The grid the routes are found on (px); coarser when the canvas is large. */
const GRID_PX = 8;
const GRID_PX_MAX = 24;
/** The most grid points one search covers; a canvas that needs more is routed straight. */
const GRID_POINTS_MAX = 40_000;
/** The share of a side (px) that ports are spread over, and the margin kept at its ends. */
const PORT_SPAN_PX = 96;
const PORT_INSET_PX = 12;
/** What each step costs beyond its length: a bend, running over a stretch another line uses, crossing a frame, hugging a frame's own border. */
const BEND_COST = 24;
const SHARED_COST = 12;
const FRAME_COST = 24;
const BORDER_COST = 4;
const BORDER_PX = 6;
/** How far (px) a search may stray beyond the box its two stubs span. */
const WINDOW_PX = 200;
/** The most lines of one selection searched on the grid; the rest take the plain elbow, so a milestone that hundreds of rules read cannot stall the page. */
const SEARCHED_LINES_MAX = 60;

/** What the router needs of a card. */
export interface RouteCard {
  key: string;
  parent?: string | undefined;
  /** Whether other cards sit inside it (a stage's frame). */
  container: boolean;
}

/** A line to route, between two cards. */
export interface RouteLine {
  from: string;
  to: string;
}

type Side = "left" | "right";
type Heading = 0 | 1 | 2 | 3; // east, south, west, north
const STEPS: readonly { x: number; y: number }[] = [{ x: 1, y: 0 }, { x: 0, y: 1 }, { x: -1, y: 0 }, { x: 0, y: -1 }];

/** The sides a line leaves its first card and enters its second by. */
function sidesOf(from: Box, to: Box): { out: Side; into: Side } {
  if (from.x + from.width <= to.x) {
    return { out: "right", into: "left" };
  }
  if (to.x + to.width <= from.x) {
    return { out: "left", into: "right" };
  }
  return { out: "left", into: "left" };
}

const portSpan = (box: Box) => Math.min(box.height, PORT_SPAN_PX);

/**
 * Each line's two ports: the lines meeting at one side of a card take evenly spaced places down
 * it, in the order of the heights of their other ends, so lines that arrive together never share
 * a point or cross at the card.
 */
function portsOf(lines: readonly RouteLine[], boxes: ReadonlyMap<string, Box>): ({ from: Point; to: Point; out: Side; into: Side} | undefined)[] {
  const meets = new Map<string, { line: number; end: "from" | "to"; other: number }[]>();
  const sides = lines.map((line) => {
    const [from, to] = [boxes.get(line.from), boxes.get(line.to)];
    return from === undefined || to === undefined ? undefined : sidesOf(from, to);
  });
  lines.forEach((line, at) => {
    const [from, to, side] = [boxes.get(line.from), boxes.get(line.to), sides[at]];
    if (from === undefined || to === undefined || side === undefined) {
      return;
    }
    const add = (key: string, end: "from" | "to", other: Box) => {
      const group = meets.get(key) ?? [];
      group.push({ line: at, end, other: other.y + other.height / 2 });
      meets.set(key, group);
    };
    add(`${line.from}/${side.out}`, "from", to);
    add(`${line.to}/${side.into}`, "to", from);
  });
  const placed = new Map<string, number>();
  for (const [key, group] of meets) {
    const box = boxes.get(key.slice(0, key.lastIndexOf("/")));
    if (box === undefined) {
      continue;
    }
    group.sort((a, b) => a.other - b.other || a.line - b.line);
    const span = portSpan(box) - 2 * PORT_INSET_PX;
    // An odd count is spread over one more place than it needs, so none falls on the middle, where a dependency leaves.
    const places = group.length % 2 === 1 ? group.length + 1 : group.length;
    group.forEach((each, rank) => placed.set(`${String(each.line)}/${each.end}`, box.y + PORT_INSET_PX + (span * (rank + 0.5)) / places));
  }
  return lines.map((line, at) => {
    const [from, to, side, start, end] = [boxes.get(line.from), boxes.get(line.to), sides[at], placed.get(`${String(at)}/from`), placed.get(`${String(at)}/to`)];
    if (from === undefined || to === undefined || side === undefined || start === undefined || end === undefined) {
      return undefined;
    }
    return {
      out: side.out,
      into: side.into,
      from: { x: side.out === "right" ? from.x + from.width : from.x, y: start },
      to: { x: side.into === "right" ? to.x + to.width : to.x, y: end },
    };
  });
}

/** A binary min-heap of numbers keyed by cost. */
class Heap {
  private readonly costs: number[] = [];
  private readonly items: number[] = [];
  get size() {
    return this.items.length;
  }
  push(cost: number, item: number) {
    let at = this.items.length;
    this.costs.push(cost);
    this.items.push(item);
    while (at > 0) {
      const up = (at - 1) >> 1;
      if ((this.costs[up] ?? 0) <= cost) {
        break;
      }
      this.swap(at, up);
      at = up;
    }
  }
  pop(): number {
    const top = this.items[0] ?? -1;
    const last = this.items.pop() ?? 0;
    const lastCost = this.costs.pop() ?? 0;
    if (this.items.length > 0) {
      this.items[0] = last;
      this.costs[0] = lastCost;
      let at = 0;
      for (;;) {
        const [a, b] = [2 * at + 1, 2 * at + 2];
        let least = at;
        if (a < this.items.length && (this.costs[a] ?? 0) < (this.costs[least] ?? 0)) least = a;
        if (b < this.items.length && (this.costs[b] ?? 0) < (this.costs[least] ?? 0)) least = b;
        if (least === at) break;
        this.swap(at, least);
        at = least;
      }
    }
    return top;
  }
  private swap(a: number, b: number) {
    [this.costs[a], this.costs[b]] = [this.costs[b] ?? 0, this.costs[a] ?? 0];
    [this.items[a], this.items[b]] = [this.items[b] ?? 0, this.items[a] ?? 0];
  }
}

const sortedUnique = (values: number[]) => [...new Set(values)].sort((a, b) => a - b);
const indexOf = (values: readonly number[], value: number) => {
  let [low, high] = [0, values.length - 1];
  while (low < high) {
    const middle = (low + high) >> 1;
    if ((values[middle] ?? 0) < value) low = middle + 1;
    else high = middle;
  }
  return low;
};

/** The corners of a route, dropping a point that lies on the straight line between its neighbours. */
function straightened(points: Point[]): Point[] {
  const out: Point[] = [];
  for (const point of points) {
    const [a, b] = [out.at(-2), out.at(-1)];
    if (b !== undefined && b.x === point.x && b.y === point.y) {
      continue;
    }
    if (a !== undefined && b !== undefined && ((a.x === b.x && b.x === point.x) || (a.y === b.y && b.y === point.y))) {
      out[out.length - 1] = point;
    } else {
      out.push(point);
    }
  }
  return out;
}


/** The grid a search runs on: the coordinates of its columns and rows. */
interface Grid {
  xs: number[];
  ys: number[];
}

/** Whether `key` lies inside the card named `above`, at any depth. */
function insideOf(parents: ReadonlyMap<string, string | undefined>, key: string, above: string): boolean {
  for (let at = parents.get(key), hops = 0; at !== undefined && hops <= parents.size; at = parents.get(at), hops += 1) {
    if (at === above) {
      return true;
    }
  }
  return false;
}

/** The grid points strictly inside `box` grown by `margin`, each handed to `apply` as its index. */
function pointsIn(grid: Grid, box: Box, margin: number, apply: (cell: number) => void) {
  const nx = grid.xs.length;
  const [x0, x1] = [indexOf(grid.xs, box.x - margin), indexOf(grid.xs, box.x + box.width + margin)];
  const [y0, y1] = [indexOf(grid.ys, box.y - margin), indexOf(grid.ys, box.y + box.height + margin)];
  for (let iy = y0; iy <= y1; iy += 1) {
    const y = grid.ys[iy] ?? 0;
    for (let ix = x0; ix <= x1; ix += 1) {
      const x = grid.xs[ix] ?? 0;
      if (x > box.x - margin && x < box.x + box.width + margin && y > box.y - margin && y < box.y + box.height + margin) {
        apply(iy * nx + ix);
      }
    }
  }
}

/** What stops or costs a route between two cards, per grid point. */
function terrainOf(grid: Grid, line: RouteLine, cards: readonly RouteCard[], boxes: ReadonlyMap<string, Box>) {
  const parents = new Map(cards.map((card) => [card.key, card.parent]));
  const size = grid.xs.length * grid.ys.length;
  const [blocked, weight] = [new Uint8Array(size), new Float32Array(size)];
  const nested = insideOf(parents, line.from, line.to) || insideOf(parents, line.to, line.from);
  const raise = (cell: number, cost: number) => {
    weight[cell] = Math.max(weight[cell] ?? 0, cost);
  };
  for (const card of cards) {
    const box = boxes.get(card.key);
    const end = card.key === line.from || card.key === line.to;
    if (box === undefined || (!end && (insideOf(parents, card.key, line.from) || insideOf(parents, card.key, line.to))) || (end && nested)) {
      continue;
    }
    if (!end && (insideOf(parents, line.from, card.key) || insideOf(parents, line.to, card.key))) {
      // A frame the line starts or ends in: crossing its border is free, running along it is not.
      pointsIn(grid, box, BORDER_PX, (cell) => {
        const [x, y] = [grid.xs[cell % grid.xs.length] ?? 0, grid.ys[Math.floor(cell / grid.xs.length)] ?? 0];
        if (x < box.x + BORDER_PX || x > box.x + box.width - BORDER_PX || y < box.y + BORDER_PX || y > box.y + box.height - BORDER_PX) {
          raise(cell, BORDER_COST);
        }
      });
    } else if (card.container && !end) {
      pointsIn(grid, box, CLEARANCE_PX / 2, (cell) => {
        raise(cell, FRAME_COST);
      });
    } else {
      pointsIn(grid, box, end ? CLEARANCE_PX / 2 : CLEARANCE_PX, (cell) => {
        blocked[cell] = 1;
      });
    }
  }
  return { blocked, weight };
}

/**
 * The cheapest path from one grid point to another, as the points where it turns, or undefined
 * when there is none. A step costs its length times the terrain's weight, a bend a fixed amount,
 * and a stretch in `used` extra; `heading` is the way the first step must not turn back on.
 */
function cheapestPath(grid: Grid, terrain: { blocked: Uint8Array; weight: Float32Array }, used: Set<number>, ends: { start: Point; goal: Point; heading: Heading }): Point[] | undefined {
  const [xs, ys] = [grid.xs, grid.ys];
  const [nx, ny] = [xs.length, ys.length];
  const [start, goal] = [indexOf(ys, ends.start.y) * nx + indexOf(xs, ends.start.x), indexOf(ys, ends.goal.y) * nx + indexOf(xs, ends.goal.x)];
  // The search stays near the stubs: a route that needs more than this much room goes round by the plain elbow.
  const window = { left: Math.min(ends.start.x, ends.goal.x) - WINDOW_PX, right: Math.max(ends.start.x, ends.goal.x) + WINDOW_PX, top: Math.min(ends.start.y, ends.goal.y) - WINDOW_PX, bottom: Math.max(ends.start.y, ends.goal.y) + WINDOW_PX };
  const cost = new Float64Array(nx * ny * 4).fill(Infinity);
  const came = new Int32Array(nx * ny * 4).fill(-1);
  const heap = new Heap();
  cost[start * 4 + ends.heading] = 0;
  heap.push(0, start * 4 + ends.heading);
  let found = -1;
  while (heap.size > 0 && found < 0) {
    const state = heap.pop();
    const [cell, heading] = [state >> 2, (state & 3) as Heading];
    if (cell === goal) {
      found = state;
      break;
    }
    const [cx, cy] = [cell % nx, Math.floor(cell / nx)];
    for (const turn of [0, 1, 3] as const) {
      const next = ((heading + turn) & 3) as Heading;
      const [mx, my] = [cx + (STEPS[next]?.x ?? 0), cy + (STEPS[next]?.y ?? 0)];
      const to = my * nx + mx;
      if (mx < 0 || my < 0 || mx >= nx || my >= ny || (terrain.blocked[to] === 1 && to !== goal) || (xs[mx] ?? 0) < window.left || (xs[mx] ?? 0) > window.right || (ys[my] ?? 0) < window.top || (ys[my] ?? 0) > window.bottom) {
        continue;
      }
      const length = Math.abs((xs[mx] ?? 0) - (xs[cx] ?? 0)) + Math.abs((ys[my] ?? 0) - (ys[cy] ?? 0));
      const reuse = used.has(Math.min(cell, to) * nx * ny + Math.max(cell, to)) ? SHARED_COST : 0;
      const spent = (cost[state] ?? Infinity) + length * (1 + Math.max(terrain.weight[cell] ?? 0, terrain.weight[to] ?? 0) + reuse) + (turn === 0 ? 0 : BEND_COST);
      if (spent < (cost[to * 4 + next] ?? Infinity)) {
        cost[to * 4 + next] = spent;
        came[to * 4 + next] = state;
        heap.push(spent + Math.abs((xs[mx] ?? 0) - ends.goal.x) + Math.abs((ys[my] ?? 0) - ends.goal.y), to * 4 + next);
      }
    }
  }
  if (found < 0) {
    return undefined;
  }
  const path: Point[] = [];
  for (let state = found; state >= 0; state = came[state] ?? -1) {
    const [cell, before] = [state >> 2, (came[state] ?? -1) >> 2];
    path.push({ x: xs[cell % nx] ?? 0, y: ys[Math.floor(cell / nx)] ?? 0 });
    if ((came[state] ?? -1) >= 0) {
      used.add(Math.min(before, cell) * nx * ny + Math.max(before, cell));
    }
  }
  return path.reverse();
}

/** A plain elbow between two points, for a canvas too large for the grid or a pair with no path. */
const elbowOf = (from: Point, to: Point): Point[] => [from, { x: (from.x + to.x) / 2, y: from.y }, { x: (from.x + to.x) / 2, y: to.y }, to];

/** The grid for these stubs over these cards: every `step` px, and at each stub's own coordinates. */
function gridOf(boxes: ReadonlyMap<string, Box>, stubs: readonly Point[]): Grid | undefined {
  const all = [...boxes.values()];
  const [xs, ys] = [[...all.flatMap((box) => [box.x, box.x + box.width]), ...stubs.map((p) => p.x)], [...all.flatMap((box) => [box.y, box.y + box.height]), ...stubs.map((p) => p.y)]];
  const [left, right, top, bottom] = [Math.min(...xs) - 4 * GRID_PX, Math.max(...xs) + 4 * GRID_PX, Math.min(...ys) - 4 * GRID_PX, Math.max(...ys) + 4 * GRID_PX];
  let step = GRID_PX;
  const points = () => ((right - left) / step + 2) * ((bottom - top) / step + 2);
  while (points() > GRID_POINTS_MAX && step < GRID_PX_MAX) {
    step += 4;
  }
  if (points() > GRID_POINTS_MAX) {
    return undefined;
  }
  const track = (from: number, to: number, extra: number[]) => sortedUnique([...Array.from({ length: Math.ceil((to - from) / step) + 1 }, (_, i) => from + i * step), ...extra]);
  return { xs: track(left, right, stubs.map((p) => p.x)), ys: track(top, bottom, stubs.map((p) => p.y)) };
}

/**
 * C1: the route of every line, in the order given, as points on the canvas from its first card to
 * its second. A route leaves and enters at its ports by a straight stub, and between the stubs
 * it is the cheapest path on a grid that avoids the cards (kept `CLEARANCE_PX` clear), prefers
 * the gutters to the inside of any stage frame that holds neither end, and avoids a stretch
 * another line already uses. A canvas too large for the grid, or two stubs that find no path,
 * are joined by a plain elbow instead.
 */
export function dateRoutes(lines: readonly RouteLine[], cards: readonly RouteCard[], boxes: ReadonlyMap<string, Box>): Point[][] {
  const ports = portsOf(lines, boxes);
  const stubOf = (port: Point, side: Side): Point => ({ x: port.x + (side === "right" ? STUB_PX : -STUB_PX), y: port.y });
  const live = ports.flatMap((port, at) => (port === undefined ? [] : [{ port, line: lines[at] as RouteLine, at, from: stubOf(port.from, port.out), to: stubOf(port.to, port.into) }]));
  const routes: Point[][] = lines.map(() => []);
  const grid = live.length === 0 ? undefined : gridOf(boxes, live.flatMap((each) => [each.from, each.to]));
  const used = new Set<number>();
  for (const [index, { port, line, at, from, to }] of live.entries()) {
    const terrain = grid === undefined || index >= SEARCHED_LINES_MAX ? undefined : terrainOf(grid, line, cards, boxes);
    const path = grid === undefined || terrain === undefined ? undefined : cheapestPath(grid, terrain, used, { start: from, goal: to, heading: port.out === "right" ? 0 : 2 });
    routes[at] = straightened([port.from, ...(path ?? elbowOf(from, to)), port.to]);
  }
  return routes;
}
