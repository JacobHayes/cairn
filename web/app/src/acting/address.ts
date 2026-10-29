// The list's own settings live in its address, as the view state does (journeys/view.ts), so a
// re-sorted list with its columns is shareable, survives a reload, and follows the back button:
// `sort=` and `cols=`. Every setting at its default leaves the address bare. The list is
// `?view=list` of a journey's address (journeys/address.ts).
import type { Schema } from "@cairn/client";

export type ListFlag = Schema<"ListFlag">;
export type SortBy = Schema<"SortBy">;

/** C9, C10: the single signals a list sorts by. */
export const SORTS: SortBy[] = ["rank", "slack", "gravity", "unlocks", "due"];

/** C9: the columns Columns adds beyond the ones every list shows, in the order they sit. */
export const LIST_COLUMNS = ["rank", "start_by", "slack", "gravity", "unlocks", "kind", "answer", "why", "affects", "effort"] as const;
export type ListColumn = (typeof LIST_COLUMNS)[number];

/** C9: the list's own settings; what it shows is the journey's view state (journeys/view.ts). */
export interface ListSettings {
  /** The one signal the rows are sorted by, which flattens the tree; none keeps plan order. */
  sort: SortBy | undefined;
  /** The columns added to the defaults. */
  columns: ListColumn[];
}

/** The values of `name` (comma-separated) that are among `known`, in `known`'s order. */
function listed<T extends string>(params: URLSearchParams, name: string, known: readonly T[]): T[] {
  const given = new Set((params.get(name) ?? "").split(","));
  return known.filter((each) => given.has(each));
}

export function listSettingsFrom(params: URLSearchParams): ListSettings {
  const sort = params.get("sort");
  return { sort: SORTS.find((each) => each === sort), columns: listed(params, "cols", LIST_COLUMNS) };
}

/** `search` with the list's own settings replaced by `list`, every other parameter kept in its order. */
export function withListSettings(search: string, list: ListSettings): string {
  const params = new URLSearchParams(search);
  params.delete("sort");
  params.delete("cols");
  if (list.sort !== undefined) {
    params.set("sort", list.sort);
  }
  if (list.columns.length > 0) {
    params.set("cols", list.columns.join(","));
  }
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}
