// The flags NEXT and CARDS filter by (`NEXT_FILTER_FLAGS`), read off the rows the engine lists.
import type { ListFlag } from "./address.ts";
import type { NodeRow } from "./why.ts";

/** Whether `row` has `flag` set. */
function flagged(row: NodeRow, flag: ListFlag): boolean {
  switch (flag) {
    case "overdue":
      return row.overdue === true;
    case "stale":
      return row.stale === true;
    case "unassigned":
      return row.unassigned === true;
    case "shortfall":
      return row.shortfall_days != null;
    default:
      return true;
  }
}

/** `rows` that have every one of `flags`. */
export function withFlags(rows: readonly NodeRow[], flags: readonly ListFlag[]): NodeRow[] {
  return rows.filter((row) => flags.every((flag) => flagged(row, flag)));
}
