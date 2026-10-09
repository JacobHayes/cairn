// C13: the timeline's rows, which follow the containment tree. The detail ladder folds them as
// it folds the graph: Stages shows the top-level stages (the current ones open to the work in
// them), Decisions adds decisions and nested groups, Work the deliverables, All the actions. A
// container that is folded, or whose children the step leaves out, is one bar from its earliest
// start to its latest due, filled by done over total. Each row says its date in plain words.
// A selected row also gets its float and its dependency lines; everything else fades. Nodes
// with no date are not rows: they gather in a list. Pure, so the unit tests check it without
// a page.
//
// Cost: O(nodes + selected dependencies) per timeline.
import { nodeOf, recordOf, type GraphNode, type NodeKind, type Ready } from "../detail/model.ts";
import { ancestorsOf, planOrder, rollups, treeOf } from "../plan/tree.ts";
import { daysWords, dueWords, daysUntil } from "../status/when.ts";
import type { DisplayState } from "../status/words.ts";
import { dateWords, type DateOrigin, type Timeline, type TimelineEntry } from "./model.ts";

/** The detail ladder's steps, coarsest first (5.1). */
export const DETAILS = ["stages", "decisions", "work", "all"] as const;
export type Detail = (typeof DETAILS)[number];

/** The kinds each step shows. A top-level node is shown whatever its kind, except at Stages, where only stages and milestones are. */
const STEP_KINDS: Record<Detail, readonly NodeKind[]> = {
  stages: ["group", "milestone"],
  decisions: ["group", "milestone", "decision"],
  work: ["group", "milestone", "decision", "deliverable"],
  all: ["group", "milestone", "decision", "deliverable", "action"],
};

/** How a row is drawn. */
export type Shape = "span" | "bar" | "tick" | "decision" | "milestone";

export interface ChartRow {
  key: string;
  title: string;
  kind: NodeKind;
  depth: number;
  state: DisplayState;
  shape: Shape;
  /** The extent on the axis: the same date for a point. */
  from: string;
  to: string;
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
  /** The selected row's float: earliest to latest start. */
  float: { from: string; to: string } | undefined;
  /** Faded: a filter excludes it, or the selection's trace does not reach it. */
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
  /** In-scope nodes with no date, not finished: the folded list. */
  undated: string[];
  dependencies: Dependency[];
  /** Every date the rows and the selected row's float lie on, for the axis. */
  dates: string[];
}

/** What the toolbar narrows the timeline to (2.3: it fades what does not match). */
export interface Narrowing {
  decisions: boolean;
  kinds: readonly NodeKind[];
  text: string;
  /** The nodes the viewer holds, when MINE is on. */
  mine: ReadonlySet<string> | undefined;
}

/** Whether a toolbar chip is narrowing the timeline at all. */
export function narrowing(narrow: Narrowing): boolean {
  return narrow.decisions || narrow.kinds.length > 0 || narrow.text.trim() !== "" || narrow.mine !== undefined;
}

/** Whether `node` is full strength under the toolbar's chips. */
export function keeps(narrow: Narrowing, node: Pick<GraphNode, "key" | "kind" | "title">): boolean {
  const wanted = narrow.text.trim().toLowerCase();
  return (
    (!narrow.decisions || node.kind === "decision") &&
    (narrow.decisions || narrow.kinds.length === 0 || narrow.kinds.includes(node.kind)) &&
    (wanted === "" || node.title.toLowerCase().includes(wanted)) &&
    (narrow.mine === undefined || narrow.mine.has(node.key))
  );
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
  detail: Detail;
  /** Containers the person has flipped from what the step would do. */
  flipped: ReadonlySet<string>;
  selected: string | undefined;
  narrow: Narrowing;
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

/** The rows the step shows, a folded or step-limited container as one bar. */
function rowsOf(view: Ready, dated: Dated, options: ChartOptions): ChartRow[] {
  const { entries, owns, spans, scopedChildren, roots } = dated;
  const rolls = rollups(view);
  const today = view.derived.today;
  // Current stages hold what is happening now: an acting-frontier or active node.
  const frontier = new Set(view.derived.acting_frontier);
  const current = (key: string): boolean => frontier.has(key) || view.derived.nodes[key]?.display_state === "active" || scopedChildren(key).some(current);
  // The selected row is always shown, under open containers; its own fold stays its own.
  const revealing = new Set(options.selected === undefined ? [] : ancestorsOf(view, options.selected));
  const forced = new Set(options.selected === undefined ? [] : [options.selected, ...revealing]);
  const rows: ChartRow[] = [];
  const visit = (key: string, depth: number, step: Detail) => {
    const node = nodeOf(view, key);
    const span = spans.get(key);
    const derived = view.derived.nodes[key];
    if (node === undefined || derived === undefined || span === undefined) {
      return;
    }
    // At Stages a current stage opens to the Work step; elsewhere the step is the same all the way down.
    const childStep: Detail = step === "stages" ? "work" : step;
    // What the step leaves out hands its dated descendants up to the nearest row that is shown.
    const shown = (keys: readonly string[]): string[] =>
      keys.flatMap((child) => {
        const kind = nodeOf(view, child)?.kind;
        return forced.has(child) || (kind !== undefined && STEP_KINDS[childStep].includes(kind)) ? [child] : shown(scopedChildren(child));
      });
    const kids = shown(scopedChildren(key)).filter((child) => spans.get(child) !== undefined);
    const container = scopedChildren(key).length > 0;
    const open = kids.length > 0 && (revealing.has(key) || (options.detail !== "stages" || current(key)) !== options.flipped.has(key));
    const own = container ? undefined : owns.get(key);
    const progress = container ? rolls.get(key) : undefined;
    const late = entries.get(key)?.overdue === true && own !== undefined ? Math.max(0, -daysUntil(own.to, today)) : 0;
    const shortfallDays = entries.get(key)?.shortfall_days ?? undefined;
    rows.push({
      key,
      title: node.title,
      kind: node.kind,
      depth,
      state: derived.display_state,
      shape: own?.shape ?? "span",
      from: own?.from ?? span.from,
      to: own?.to ?? span.to,
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
      kids.forEach((child) => { visit(child, depth + 1, childStep); });
    }
  };
  // Stages is the calm view: top-level stages and milestones, the decisions arriving with their step.
  const wanted = (root: string) => {
    const kind = nodeOf(view, root)?.kind;
    return options.detail !== "stages" || forced.has(root) || (kind !== undefined && STEP_KINDS.stages.includes(kind));
  };
  roots.filter((root) => spans.get(root) !== undefined && wanted(root)).forEach((root) => { visit(root, 0, options.detail); });
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
    own.float = { from: dates.earliest_start.date, to: dates.latest_start.date };
  }
  return { row, dependencies };
}

export function chartOf(view: Ready, timeline: Timeline, options: ChartOptions): Chart {
  const dated = datedOf(view, timeline);
  const rows = rowsOf(view, dated, options);
  const { row: selected, dependencies } = options.selected === undefined ? { row: undefined, dependencies: [] } : selectionOf(view, rows, options.selected);

  // Fading (2.3): with a selection the trace decides; otherwise the toolbar's chips do.
  const traced = new Set(dependencies.flatMap((each) => [each.from, each.to]));
  if (selected !== undefined) {
    traced.add(selected);
  }
  const matches = new Map<string, boolean>();
  const matched = (key: string): boolean => {
    let found = matches.get(key);
    if (found === undefined) {
      const node = nodeOf(view, key);
      found = (node !== undefined && keeps(options.narrow, node)) || dated.scopedChildren(key).some(matched);
      matches.set(key, found);
    }
    return found;
  };
  const narrowed = narrowing(options.narrow);
  for (const row of rows) {
    row.faded = selected === undefined ? narrowed && !matched(row.key) : !traced.has(row.key);
  }

  const undated = planOrder(view).filter((key) => {
    const state = view.derived.nodes[key]?.display_state;
    return dated.inScope(key) && dated.spans.get(key) === undefined && dated.scopedChildren(key).length === 0 && state !== "done" && state !== "skipped";
  });
  return { rows, undated, dependencies, dates: rows.flatMap((row) => [row.from, row.to, ...(row.float === undefined ? [] : [row.float.from, row.float.to])]) };
}
