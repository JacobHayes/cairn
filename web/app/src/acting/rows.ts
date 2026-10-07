// C9's grouping by container, and the breadcrumb every acting surface shows (C10): a row's
// ancestors, root first, as the engine's row lists them.
import type { NodeRow } from "./why.ts";

/** A run of rows under one container, in the list's sort order. */
export interface RowGroup {
  /** The nearest container the rows share; none for nodes at the top of the journey. */
  container: string | undefined;
  /** Its ancestors and itself, root first: its place in the tree. */
  path: string[];
  rows: NodeRow[];
}

/**
 * C9: `rows` grouped by their nearest container, each group where its first row falls in the
 * sort order, rows within it keeping that order.
 */
export function byContainer(rows: readonly NodeRow[]): RowGroup[] {
  const groups = new Map<string, RowGroup>();
  for (const row of rows) {
    const ancestors = row.ancestors ?? [];
    const container = ancestors.at(-1);
    const id = container ?? "";
    let group = groups.get(id);
    if (group === undefined) {
      group = { container, path: ancestors, rows: [] };
      groups.set(id, group);
    }
    group.rows.push(row);
  }
  return [...groups.values()];
}
