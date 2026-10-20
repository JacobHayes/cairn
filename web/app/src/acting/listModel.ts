// C9: the Plan list as data. The journey's level (journeys/requests.ts) says which nodes are
// drawn, and under which; the engine's list gives each its row. This lays them out: the tree by
// default (the level's order, each node under the drawn node it is drawn under, folded as the
// ladder and the viewer say), or flat in the sort's order once a column is sorted, which gives
// each row its container under its title. Also the columns, in the order they sit. Pure, so the
// unit tests check it without a page.
import type { Ready } from "../detail/model.ts";
import { planOrder, treeOf } from "../plan/tree.ts";
import { isOpen } from "../status/state.ts";
import { LIST_COLUMNS, type ListColumn, type ListSettings, type SortBy } from "./address.ts";
import type { NodeRow } from "./why.ts";

/** Everything a column can be: the ones every list has, then the ones Columns adds. */
export type Column = "change" | "status" | "title" | "owner" | "due" | ListColumn;

/** What Columns can add: the decision columns are already there at Decisions depth, and the rows are mixed, so Kind stays. */
export function addableColumns(decisionColumns: boolean): readonly ListColumn[] {
  return decisionColumns ? LIST_COLUMNS.filter((column) => !["answer", "why", "affects"].includes(column)) : LIST_COLUMNS;
}

/** Whether the rows are in rank order, which puts the Rank column first: a sort by rank. */
export function ranked(list: ListSettings): boolean {
  return list.sort === "rank";
}

/** C9, C12: the columns shown, left to right; at Decisions depth (`decisionColumns`) the decision columns after Title. */
export function columnsOf(list: ListSettings, decisionColumns: boolean, layered = false): Column[] {
  const chosen = addableColumns(decisionColumns).filter((column) => list.columns.includes(column) && !(column === "rank" && ranked(list)));
  const own: Column[] = decisionColumns ? ["status", "title", "answer", "why", "affects", "owner", "due"] : ["status", "title", "owner", "due"];
  return [...(layered ? (["change"] as const) : []), ...(ranked(list) ? (["rank"] as const) : []), ...own, ...chosen];
}

/** The column words. */
export function columnWords(column: Column): string {
  switch (column) {
    case "start_by":
      return "Start by";
    case "change":
      return "Change";
    default:
      return column.charAt(0).toUpperCase() + column.slice(1);
  }
}

/** Whether a column gives way when the region is narrow: only the decision's why, a long sentence; the rest the person asked for, so the table scrolls to them. */
export function givesWay(column: Column): boolean {
  return column === "why";
}

/** The signal a column sorts by; the others have none. */
export const SORT_OF: Partial<Record<Column, SortBy>> = {
  rank: "rank",
  slack: "slack",
  gravity: "gravity",
  unlocks: "unlocks",
  due: "due",
  effort: "effort",
};

/** One line of the table: a node the level draws, with its row. */
export interface Line {
  key: string;
  /** How far in the tree it sits; every flat line is 0. */
  depth: number;
  row: NodeRow | undefined;
  /** It is a container of the graph (`containerKeys`), whether or not any of what it holds is drawn. */
  container: boolean;
  /** It has drawn lines beneath it, shown or folded: the fold button's. */
  foldable: boolean;
  open: boolean;
  /**
   * The tree's guide lines, one per level: at `depth - 1` (the last) the row's own `├` or `└`,
   * before it each ancestor's line. True while that level's parent has more children below.
   */
  guides: boolean[];
  /** A row the proposal removes: its cells are read from the journey as it is, not the one after. */
  from?: Ready | undefined;
}

/**
 * C2, C9: the tree of the drawn nodes (`drawn`, the level's nodes, each with the drawn node it is
 * drawn under), siblings in plan order (`order`, each key's place), each container shown open or
 * folded as `open` says. A drawn node with no row (read at another revision for a moment) is left
 * out, its children kept under the nearest drawn node above it that has one.
 */
export function treeLines(drawn: readonly { key: string; parent?: string | null }[], rows: ReadonlyMap<string, NodeRow>, open: (container: string) => boolean, order: ReadonlyMap<string, number>, containers: ReadonlySet<string>): Line[] {
  const parents = new Map(drawn.map((node) => [node.key, node.parent ?? undefined]));
  const shownAbove = (key: string): string | undefined => {
    let at = parents.get(key);
    while (at !== undefined && !rows.has(at)) {
      at = parents.get(at);
    }
    return at;
  };
  const children = new Map<string | undefined, string[]>();
  for (const node of drawn) {
    if (rows.has(node.key)) {
      const parent = shownAbove(node.key);
      children.set(parent, [...(children.get(parent) ?? []), node.key]);
    }
  }
  const place = (key: string) => order.get(key) ?? Number.MAX_SAFE_INTEGER;
  for (const kids of children.values()) {
    kids.sort((left, right) => place(left) - place(right));
  }
  const lines: Line[] = [];
  const visit = (key: string, depth: number, guides: boolean[]) => {
    const held = children.get(key) ?? [];
    const isOpen = held.length > 0 && open(key);
    lines.push({ key, depth, row: rows.get(key), container: containers.has(key), foldable: held.length > 0, open: isOpen, guides });
    if (isOpen) {
      held.forEach((child, at) => { visit(child, depth + 1, [...guides, at < held.length - 1]); });
    }
  };
  (children.get(undefined) ?? []).forEach((root) => { visit(root, 0, []); });
  return lines;
}

/**
 * C9: `rows` flat: in the order the engine sorted them by `sort`, or in plan order when none is.
 * Sorted by a date, the rows that are over come after the open ones: their date cells are blank.
 * Sorted by any signal, the snoozed rows come after the unsnoozed ones, in their own order: they
 * are not now (the shared rank itself is unchanged; this is display order).
 */
export function flatLines(view: Ready, rows: readonly NodeRow[], sort: SortBy | undefined): Line[] {
  const order = new Map(planOrder(view).map((key, at) => [key, at]));
  const snoozed = (row: NodeRow) => sort !== undefined && row.display_state === "snoozed";
  const planned = sort === undefined ? [...rows].sort((left, right) => (order.get(left.key) ?? 0) - (order.get(right.key) ?? 0)) : rows;
  const ordered = [...planned.filter((row) => !snoozed(row)), ...planned.filter(snoozed)];
  const over = (row: NodeRow) => (sort === "due" || sort === "slack") && !isOpen(view.derived.nodes[row.key]);
  return [...ordered.filter((row) => !over(row)), ...ordered.filter(over)].map((row) => ({ key: row.key, depth: 0, row, container: false, foldable: false, open: false, guides: [] }));
}

/** Whether node `key` holds other nodes. */
export function isContainer(view: Ready, key: string): boolean {
  return (treeOf(view).children.get(key) ?? []).length > 0;
}

/** B2, C12: a rationale's first paragraph as plain text, for a one-line cell. */
export function firstParagraph(markdown: string): string {
  const paragraph = markdown.trim().split(/\n\s*\n/)[0] ?? "";
  return paragraph
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}(#{1,6}|[-*+]|\d+\.|>)\s+/gm, "")
    .replace(/[*_`~]+/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
