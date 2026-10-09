// C9: the Plan list as data. The engine's list answers which nodes match; this lays them out:
// the tree by default (plan order, each container with what matches beneath it, folded to the
// top level), or flat in the sort's order once a column is sorted, which gives each row its
// container under its title. Also the columns, in the order they sit. Pure, so the unit tests
// check it without a page.
import type { Ready } from "../detail/model.ts";
import { ancestorsOf, planOrder, treeOf } from "../plan/tree.ts";
import { LIST_COLUMNS, type ListColumn, type ListSettings, type SortBy } from "./address.ts";
import type { NodeRow } from "./why.ts";

/** Everything a column can be: the ones every list has, then the ones Columns adds. */
export type Column = "status" | "title" | "owner" | "due" | ListColumn;

/** What Columns can add: the decision columns are already there with DECISIONS on, and a decision's kind is no news. */
export function addableColumns(decisions: boolean): readonly ListColumn[] {
  return decisions ? LIST_COLUMNS.filter((column) => !["kind", "answer", "why", "affects"].includes(column)) : LIST_COLUMNS;
}

/** C9, C12: the columns shown, left to right. */
export function columnsOf(settings: ListSettings): Column[] {
  const chosen = addableColumns(settings.decisions).filter((column) => settings.columns.includes(column));
  return settings.decisions ? ["status", "title", "answer", "why", "affects", "owner", "due", ...chosen] : ["status", "title", "owner", "due", ...chosen];
}

/** The column words; a decision list says Decision and Decide by. */
export function columnWords(column: Column, decisions: boolean): string {
  switch (column) {
    case "title":
      return decisions ? "Decision" : "Title";
    case "due":
      return decisions ? "Decide by" : "Due";
    case "start_by":
      return "Start by";
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
  unblocks: "leverage",
  due: "due",
  effort: "effort",
};

/** One line of the table: a row the engine matched, or a container kept for what matches beneath it. */
export interface Line {
  key: string;
  /** How far in the tree it sits; every flat line is 0. */
  depth: number;
  row: NodeRow | undefined;
  /** It holds lines of its own, shown or folded. */
  container: boolean;
  open: boolean;
}

/**
 * C9: the tree of `rows`: every container above a match is there to hold it, shown or folded
 * as `open` says, the top level always.
 */
export function treeLines(view: Ready, rows: readonly NodeRow[], open: (container: string) => boolean): Line[] {
  const matched = new Map(rows.map((row) => [row.key, row]));
  const needed = new Set(matched.keys());
  for (const row of rows) {
    ancestorsOf(view, row.key).forEach((ancestor) => needed.add(ancestor));
  }
  const { roots, children } = treeOf(view);
  const lines: Line[] = [];
  const visit = (key: string, depth: number) => {
    const held = (children.get(key) ?? []).filter((child) => needed.has(child));
    const isOpen = held.length > 0 && open(key);
    lines.push({ key, depth, row: matched.get(key), container: held.length > 0, open: isOpen });
    if (isOpen) {
      held.forEach((child) => { visit(child, depth + 1); });
    }
  };
  roots.filter((root) => needed.has(root)).forEach((root) => { visit(root, 0); });
  return lines;
}

/**
 * C9: `rows` flat: in the order the engine sorted them by `sort`, or in plan order when none is.
 * Sorted by a date, the rows that are over come after the open ones: their date cells are blank.
 */
export function flatLines(view: Ready, rows: readonly NodeRow[], sort: SortBy | undefined): Line[] {
  const order = new Map(planOrder(view).map((key, at) => [key, at]));
  const ordered = sort === undefined ? [...rows].sort((left, right) => (order.get(left.key) ?? 0) - (order.get(right.key) ?? 0)) : rows;
  const over = (row: NodeRow) => (sort === "due" || sort === "slack") && ["done", "skipped", "not_relevant"].includes(row.display_state);
  return [...ordered.filter((row) => !over(row)), ...ordered.filter(over)].map((row) => ({ key: row.key, depth: 0, row, container: false, open: false }));
}

/** C10: each ranked node's place in the journey's order, by rank, best first (`#1`). */
export function rankPositions(view: Ready): Map<string, number> {
  const ranked = Object.entries(view.derived.nodes).flatMap(([key, derived]) => (derived.rank == null ? [] : [{ key, rank: derived.rank }]));
  ranked.sort((left, right) => right.rank - left.rank || (left.key < right.key ? -1 : 1));
  return new Map(ranked.map(({ key }, at) => [key, at + 1]));
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
