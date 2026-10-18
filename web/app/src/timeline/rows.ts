// C13: the timeline's rows: the nodes the journey's level draws (journeys/requests.ts: Depth's
// step, the Filter and the search, as every view draws them) that have dates, each under the
// drawn node it is drawn under, folded as the ladder says and the viewer flips (at Stages only
// the current stages open). A container that is folded, or whose children the level leaves out,
// is one bar from its earliest start to its latest due, filled by done over total. Each row says
// its date in plain words. A selected row also gets its float and its dependency lines, and
// what its trace does not reach fades; so does what is out of focus for Frontier or for Depth's
// Fade, the lowest opacity winning. Drawn nodes with no date are not rows: they gather in a list. Pure,
// so the unit tests check it without a page.
//
// Cost: O(nodes + selected dependencies) per timeline.
import type { RouteSchedule, ScheduleEntry } from "@cairn/wasm";

import { childrenOf, nodesByPath, titleIn, type Tree } from "../authoring/graph.ts";
import { nodeOf, recordOf, type GraphNode, type NodeKind, type Ready } from "../detail/model.ts";
import { ancestorsOf, planOrder, rollups, treeOf } from "../plan/tree.ts";
import { daysWords, dueWords, daysUntil } from "../status/when.ts";
import type { DisplayState } from "../status/words.ts";
import { dateWords, dayOf, dayWords, type DateOrigin, type Timeline, type TimelineEntry } from "./model.ts";

/** How a row is drawn. */
export type Shape = "span" | "bar" | "tick" | "decision" | "milestone";

export interface ChartRow {
  key: string;
  title: string;
  kind: NodeKind;
  depth: number;
  /** The tree's guide lines (ui/Tree.tsx): at `depth - 1` the row's own elbow, before it each ancestor's line, true while that level's parent has more rows below. */
  guides: boolean[];
  state: DisplayState;
  shape: Shape;
  /** The extent on the axis, in days: the same day for a point. */
  from: number;
  to: number;
  /** A milestone's date and where it comes from. */
  origin: DateOrigin | undefined;
  /** Its date in plain words: "Due in 3 days", "3 days late", "Pinned Oct 30". */
  words: string;
  /** Days past due, when it is. */
  late: number;
  shortfallDays: number | undefined;
  /** The `final` milestone: the end anchor. */
  end: boolean;
  /** How far a container is. */
  progress: { done: number; total: number } | undefined;
  /** It has rows of its own beneath it that can be opened or folded. */
  foldable: boolean;
  open: boolean;
  /** The selected row's float, in days: earliest to latest start. */
  float: { from: number; to: number } | undefined;
  /** Faded: the selection's trace does not reach it, or a highlight does not light it. */
  faded: boolean;
}

/** A line between two rows: what the selected row needs, what it unblocks, or the chain it is short on. */
export interface Dependency {
  from: string;
  to: string;
  kind: "needs" | "unblocks" | "short";
}

export interface Chart {
  rows: ChartRow[];
  /** In-scope nodes with no date, not finished, and the selected node when no row stands for it: the folded list. */
  undated: string[];
  /** What a node in the folded list says about why it has no day (a route's rule waiting on a decision). */
  undatedWords?: ReadonlyMap<string, string>;
  dependencies: Dependency[];
  /** Every day the rows and the selected row's float lie on, for the axis. */
  days: number[];
}

interface Extent {
  from: string;
  to: string;
}

const earlier = (left: string, right: string) => (left <= right ? left : right);
const later = (left: string, right: string) => (left >= right ? left : right);

function union(left: Extent | undefined, right: Extent | undefined): Extent | undefined {
  return left === undefined ? right : right === undefined ? left : { from: earlier(left.from, right.from), to: later(left.to, right.to) };
}

interface Own extends Extent {
  shape: Shape;
  origin: DateOrigin | undefined;
  entry: TimelineEntry | undefined;
  /** When the work could start at the soonest, which a container's bar starts from; its own bar starts at the latest start. */
  earliest: string | undefined;
}

/** The dates a node has of its own: its milestone date, its decide-by, or its work's start and due. */
function ownOf(view: Ready, node: GraphNode, entry: TimelineEntry | undefined): Own | undefined {
  const record = recordOf(view, node);
  const finished = record.finished_on ?? undefined;
  const at = entry?.date ?? finished;
  if (at === undefined) {
    return undefined;
  }
  if (node.kind === "milestone") {
    return { from: at, to: at, shape: "milestone", origin: entry?.origin ?? "actual", entry, earliest: undefined };
  }
  if (node.kind === "decision") {
    return { from: at, to: at, shape: "decision", origin: undefined, entry, earliest: undefined };
  }
  const derived = view.derived.nodes[node.key];
  const started = derived?.display_state === "done" ? (record.started_on ?? undefined) : derived?.dates.latest_start?.date;
  const from = started !== undefined && started < at ? started : at;
  const soonest = derived?.display_state === "done" ? undefined : derived?.dates.earliest_start?.date;
  return { from, to: at, shape: from < at ? "bar" : "tick", origin: entry?.origin, entry, earliest: soonest !== undefined && soonest < from ? soonest : undefined };
}

function wordsOf(view: Ready, node: GraphNode, own: Own | undefined, progress: ChartRow["progress"], late: number): string {
  const today = view.derived.today;
  if (own === undefined || own.shape === "span") {
    return progress === undefined || progress.total === 0 ? "" : `${String(progress.done)} of ${String(progress.total)} done`;
  }
  const finished = recordOf(view, node).finished_on ?? undefined;
  switch (own.shape) {
    case "milestone":
      return late > 0 ? `${daysWords(late)} late` : own.origin === "actual" ? `Reached ${dateWords(own.to, today)}` : own.origin === "pin" ? `Pinned ${dateWords(own.to, today)}` : `Estimated ${dateWords(own.to, today)}`;
    case "decision":
      return finished !== undefined && own.entry === undefined ? `Decided ${dateWords(finished, today)}` : dueWords(own.to, today, true);
    default:
      return finished !== undefined && late === 0 && view.derived.nodes[node.key]?.display_state === "done" ? `Finished ${dateWords(finished, today)}` : dueWords(own.to, today);
  }
}

export interface ChartOptions {
  /** The level's nodes, each with the drawn node it is drawn under. */
  drawn: readonly { key: string; parent?: string | null }[];
  /** Whether a container is open: the step's rule with the viewer's flips, and open above the selected node (canvas/hooks.ts, `useFolds`). */
  open: (key: string) => boolean;
  selected: string | undefined;
  /** Whether a row is out of focus for Frontier or for Depth's Fade (journeys/requests.ts, `outOfFocus`). */
  faded: (key: string) => boolean;
}

/** What the journey dates: each in-scope node's own dates, and what each one's whole subtree spans. */
interface Dated {
  entries: Map<string, TimelineEntry>;
  owns: Map<string, Own | undefined>;
  spans: Map<string, Extent | undefined>;
  inScope: (key: string) => boolean;
  scopedChildren: (key: string) => string[];
  roots: string[];
}

function datedOf(view: Ready, timeline: Timeline): Dated {
  const tree = treeOf(view);
  const entries = new Map(timeline.entries.map((entry) => [entry.node, entry]));
  const inScope = (key: string) => {
    const state = view.derived.nodes[key]?.display_state;
    return state !== undefined && state !== "not_relevant";
  };
  // A node ruled out leaves its in-scope descendants (force-included work) under its own parent.
  const inScopeIn = (keys: readonly string[]): string[] => keys.flatMap((key) => (inScope(key) ? [key] : inScopeIn(tree.children.get(key) ?? [])));
  const scopedChildren = (key: string) => inScopeIn(tree.children.get(key) ?? []);
  const owns = new Map<string, Own | undefined>();
  const spans = new Map<string, Extent | undefined>();
  const spanOf = (key: string): Extent | undefined => {
    if (spans.has(key)) {
      return spans.get(key);
    }
    const node = nodeOf(view, key);
    const own = node === undefined ? undefined : ownOf(view, node, entries.get(key));
    owns.set(key, own);
    const reach = own === undefined ? undefined : { from: own.earliest ?? own.from, to: own.to };
    const span = scopedChildren(key).reduce<Extent | undefined>((all, child) => union(all, spanOf(child)), reach);
    spans.set(key, span);
    return span;
  };
  const roots = inScopeIn(tree.roots);
  roots.forEach(spanOf);
  return { entries, owns, spans, inScope, scopedChildren, roots };
}

/** The drawn nodes that are rows (those with dates), each under the nearest of them it is drawn beneath, siblings in plan order. */
function treeOfRows(view: Ready, dated: Dated, drawn: ChartOptions["drawn"]): Map<string | undefined, string[]> {
  const parents = new Map(drawn.map((node) => [node.key, node.parent ?? undefined]));
  const dates = (key: string) => dated.spans.get(key) !== undefined;
  const above = (key: string): string | undefined => {
    let at = parents.get(key);
    while (at !== undefined && !dates(at)) {
      at = parents.get(at);
    }
    return at;
  };
  const order = new Map(planOrder(view).map((key, at) => [key, at]));
  const place = (key: string) => order.get(key) ?? Number.MAX_SAFE_INTEGER;
  const children = new Map<string | undefined, string[]>();
  for (const { key } of drawn) {
    if (dates(key)) {
      const parent = above(key);
      children.set(parent, [...(children.get(parent) ?? []), key]);
    }
  }
  for (const kids of children.values()) {
    kids.sort((left, right) => place(left) - place(right));
  }
  return children;
}

/** The rows the level draws, a folded or step-limited container as one bar. */
function rowsOf(view: Ready, dated: Dated, options: ChartOptions): ChartRow[] {
  const { entries, owns, spans, scopedChildren } = dated;
  const rolls = rollups(view);
  const today = view.derived.today;
  const children = treeOfRows(view, dated, options.drawn);
  const rows: ChartRow[] = [];
  const visit = (key: string, depth: number, guides: boolean[]) => {
    const node = nodeOf(view, key);
    const span = spans.get(key);
    const derived = view.derived.nodes[key];
    if (node === undefined || derived === undefined || span === undefined) {
      return;
    }
    const kids = children.get(key) ?? [];
    const container = scopedChildren(key).length > 0;
    const open = kids.length > 0 && options.open(key);
    const own = container ? undefined : owns.get(key);
    const progress = container ? rolls.get(key) : undefined;
    const late = entries.get(key)?.overdue === true && own !== undefined ? Math.max(0, -daysUntil(own.to, today)) : 0;
    const shortfallDays = entries.get(key)?.shortfall_days ?? undefined;
    rows.push({
      key,
      title: node.title,
      kind: node.kind,
      depth,
      guides,
      state: derived.display_state,
      shape: own?.shape ?? "span",
      from: dayOf(own?.from ?? span.from),
      to: dayOf(own?.to ?? span.to),
      origin: own?.origin,
      words: [wordsOf(view, node, own, progress, late), shortfallDays === undefined ? "" : `${daysWords(shortfallDays)} short`].filter(Boolean).join(", "),
      late,
      shortfallDays,
      end: entries.get(key)?.final === true,
      progress,
      foldable: kids.length > 0,
      open,
      float: undefined,
      faded: false,
    });
    if (open) {
      kids.forEach((child, at) => { visit(child, depth + 1, [...guides, at < kids.length - 1]); });
    }
  };
  (children.get(undefined) ?? []).forEach((root) => { visit(root, 0, []); });
  return rows;
}

/** The nodes a chain of constraints runs through, in order, each once. */
function chainNodes(view: Ready, key: string): string[] {
  const chain = view.derived.nodes[key]?.dates.shortfall?.chain;
  const keys: string[] = [];
  for (const { before, after } of chain?.constraints ?? []) {
    for (const instant of [before, after]) {
      if (typeof instant === "object" && "node" in instant && !keys.includes(instant.node.node)) {
        keys.push(instant.node.node);
      }
    }
  }
  return keys;
}

/** The selected row: its float, and the lines to what it needs and unblocks, or the chain it is short on. */
function selectionOf(view: Ready, rows: ChartRow[], selected: string): { row: string | undefined; dependencies: Dependency[] } {
  const shown = new Set(rows.map((row) => row.key));
  const rowFor = (key: string): string | undefined => [key, ...ancestorsOf(view, key).reverse()].find((each) => shown.has(each));
  const row = rowFor(selected);
  const dependencies: Dependency[] = [];
  const draw = (from: string | undefined, to: string | undefined, kind: Dependency["kind"]) => {
    if (from !== undefined && to !== undefined && from !== to && !dependencies.some((each) => each.from === from && each.to === to && each.kind === kind)) {
      dependencies.push({ from, to, kind });
    }
  };
  if (row === undefined) {
    return { row, dependencies };
  }
  // What a node needs: the requirements it names, and what still holds it (inherited, condition gates).
  const nodes = view.journey.graph.nodes ?? [];
  const holds = (key: string): string[] => [
    ...(nodes.find((each) => each.key === key)?.requires ?? []),
    ...(view.derived.nodes[key]?.blocked_by ?? []).filter((blocker) => blocker.via !== "containment").map((blocker) => blocker.node),
  ];
  for (const need of holds(selected)) {
    draw(rowFor(need), row, "needs");
  }
  for (const { key } of nodes) {
    if (holds(key).includes(selected)) {
      draw(row, rowFor(key), "unblocks");
    }
  }
  const chain = chainNodes(view, selected).map(rowFor);
  chain.slice(1).forEach((to, at) => { draw(chain[at], to, "short"); });
  const dates = view.derived.nodes[selected]?.dates;
  const own = rows.find((each) => each.key === row);
  if (own !== undefined && row === selected && dates?.earliest_start != null && dates.latest_start != null && dates.earliest_start.date < dates.latest_start.date) {
    own.float = { from: dayOf(dates.earliest_start.date), to: dayOf(dates.latest_start.date) };
  }
  return { row, dependencies };
}

/** C13: the chart of the drawn nodes: their rows, the selection's float and lines, what fades, and the drawn work with no date. */
export function chartOf(view: Ready, timeline: Timeline, options: ChartOptions): Chart {
  const dated = datedOf(view, timeline);
  const rows = rowsOf(view, dated, options);
  const { row: selected, dependencies } = options.selected === undefined ? { row: undefined, dependencies: [] } : selectionOf(view, rows, options.selected);

  // Out of focus (2.3) is outside the selection's trace or outside the highlight: the lowest opacity wins.
  const traced = new Set(dependencies.flatMap((each) => [each.from, each.to]));
  if (selected !== undefined) {
    traced.add(selected);
  }
  for (const row of rows) {
    row.faded = (selected !== undefined && !traced.has(row.key)) || options.faded(row.key);
  }

  const drawn = new Set(options.drawn.map((node) => node.key));
  const undated = planOrder(view).filter((key) => {
    const state = view.derived.nodes[key]?.display_state;
    const dateless = dated.spans.get(key) === undefined && dated.scopedChildren(key).length === 0;
    // A selected node the chart has no row for is listed whatever its state or scope: it has no date (C13), and it is there to be seen (C7).
    if (key === options.selected && !rows.some((row) => row.key === key)) {
      return true;
    }
    return drawn.has(key) && dated.inScope(key) && dateless && state !== "done" && state !== "skipped";
  });
  return {
    rows,
    undated,
    dependencies,
    days: rows.flatMap((row) => [row.from, row.to, ...(row.float === undefined ? [] : [row.float.from, row.float.to])]),
  };
}

/** The days a node and what lies beneath it reach: from the earliest start to the latest finish. */
interface Reach {
  from: number;
  to: number;
}

const reachUnion = (left: Reach | undefined, right: Reach | undefined): Reach | undefined =>
  left === undefined ? right : right === undefined ? left : { from: Math.min(left.from, right.from), to: Math.max(left.to, right.to) };

/** A route row's days in words: "Day 7" for a point, "Days 3 to 7" for a stretch. */
function routeWords(reach: Reach): string {
  return reach.from === reach.to ? dayWords(reach.from) : `Days ${String(reach.from)} to ${String(reach.to)}`;
}

/** The days each node of a route spans: its own earliest start and finish, or for a container the span of what it holds. */
function reachesOf(tree: Tree, entries: ReadonlyMap<string, ScheduleEntry>): (key: string) => Reach | undefined {
  const reaches = new Map<string, Reach | undefined>();
  const reachOf = (key: string): Reach | undefined => {
    if (!reaches.has(key)) {
      const kids = childrenOf(tree, key);
      const entry = entries.get(key);
      const own = entry === undefined ? undefined : { from: entry.start_day, to: entry.finish_day };
      // A stage is its children's span; a deliverable or action with children also has its own work, after theirs (PRD, Containment), so its own days count too.
      const beneath = kids.reduce<Reach | undefined>((all, kid) => reachUnion(all, reachOf(kid.key)), undefined);
      const stage = tree.byKey.get(key)?.kind === "group";
      reaches.set(key, kids.length > 0 && beneath !== undefined ? (stage ? beneath : reachUnion(beneath, own)) : own);
    }
    return reaches.get(key);
  };
  return reachOf;
}

/** One row of a route's chart: `key` at `depth`, spanning `reach`, with `kids` drawn under it when it is `open`. */
function routeRowOf(tree: Tree, schedule: RouteSchedule, options: ChartOptions, at: { key: string; depth: number; guides: boolean[]; reach: Reach; entry: ScheduleEntry | undefined; kids: number; open: boolean }): ChartRow {
  const { key, depth, guides, reach, entry } = at;
  const node = tree.byKey.get(key);
  const shape: Shape = childrenOf(tree, key).length > 0 ? "span" : node?.kind === "milestone" ? "milestone" : node?.kind === "decision" ? "decision" : reach.from < reach.to ? "bar" : "tick";
  const waiting = (entry?.waiting ?? []).map((decision) => titleIn(tree, decision));
  return {
    key,
    title: node?.title ?? key,
    kind: node?.kind ?? "action",
    depth,
    guides,
    state: entry?.conditional === true ? "conditional" : "ready",
    shape,
    from: reach.from,
    to: reach.to,
    origin: undefined,
    words: [routeWords(reach), waiting.length === 0 ? "" : `waits on ${waiting.join(", ")}`].filter(Boolean).join(", "),
    late: 0,
    shortfallDays: undefined,
    end: key === schedule.end,
    progress: undefined,
    foldable: at.kids > 0,
    open: at.open,
    float: undefined,
    faded: options.faded(key),
  };
}

/**
 * C13: a route's chart: the nodes the level draws, each at the earliest days the schedule gives it
 * (from the journey's start, day 0), under the drawn node it is drawn under, folded as `open`
 * says. A container is one span over what lies beneath it, siblings go by their earliest start
 * (the bars are the before and after order) and then by the graph's own order, and a node
 * inside an undecided branch is drawn conditional. A row whose date rule waits on a date decision
 * nothing answers says so, since its day is the earliest the rules allow. Drawn nodes with no
 * earliest day gather in the folded list. Pure, so the unit tests check it without a page.
 *
 * Cost: O(nodes log nodes) per route.
 */
export function routeChartOf(tree: Tree, schedule: RouteSchedule, options: ChartOptions): Chart {
  const entries = new Map(schedule.entries.map((entry) => [entry.node, entry]));
  const order = new Map(nodesByPath(tree).map((node, at) => [node.key, at]));
  const place = (key: string) => order.get(key) ?? Number.MAX_SAFE_INTEGER;
  const reachOf = reachesOf(tree, entries);
  const parents = new Map(options.drawn.map((node) => [node.key, node.parent ?? undefined]));
  const dated = (key: string) => reachOf(key) !== undefined;
  const above = (key: string): string | undefined => {
    let at = parents.get(key);
    while (at !== undefined && !dated(at)) {
      at = parents.get(at);
    }
    return at;
  };
  const children = new Map<string | undefined, string[]>();
  for (const { key } of options.drawn) {
    if (dated(key)) {
      const parent = above(key);
      children.set(parent, [...(children.get(parent) ?? []), key]);
    }
  }
  for (const kids of children.values()) {
    kids.sort((left, right) => (reachOf(left)?.from ?? 0) - (reachOf(right)?.from ?? 0) || (reachOf(left)?.to ?? 0) - (reachOf(right)?.to ?? 0) || place(left) - place(right));
  }
  const rows: ChartRow[] = [];
  const visit = (key: string, depth: number, guides: boolean[]) => {
    const reach = reachOf(key);
    if (!tree.byKey.has(key) || reach === undefined) {
      return;
    }
    const kids = children.get(key) ?? [];
    const open = kids.length > 0 && options.open(key);
    rows.push(routeRowOf(tree, schedule, options, { key, depth, guides, reach, entry: entries.get(key), kids: kids.length, open }));
    if (open) {
      kids.forEach((child, at) => { visit(child, depth + 1, [...guides, at < kids.length - 1]); });
    }
  };
  (children.get(undefined) ?? []).forEach((root) => { visit(root, 0, []); });
  const drawn = new Set(options.drawn.map((node) => node.key));
  const gaps = (schedule.undated ?? []).filter((gap) => drawn.has(gap.node));
  const undated = gaps.map((gap) => gap.node);
  const undatedWords = new Map(gaps.filter((gap) => (gap.waiting ?? []).length > 0).map((gap) => [gap.node, `waits on ${(gap.waiting ?? []).map((decision) => titleIn(tree, decision)).join(", ")}`] as const));
  // A selected node the chart has no row for is listed whatever it has: it is there to be seen (C7).
  const lost = options.selected !== undefined && !rows.some((row) => row.key === options.selected) && !undated.includes(options.selected) && tree.byKey.has(options.selected);
  return { rows, undated: lost && options.selected !== undefined ? [...undated, options.selected] : undated, undatedWords, dependencies: [], days: rows.flatMap((row) => [row.from, row.to]) };
}
