// C9: every row the list's query matches, read a page after another from the tab's derivation
// (the engine answers `page_item_count_max` rows at a time) until there are no more. The tree
// needs them all to put each match under its container. Like the canvas's reads it keeps the
// last answer for the same request until the next arrives, so a live edit swaps the rows once.
import type { Schema } from "@cairn/client";
import type { ListQuery } from "@cairn/wasm";

import { PROJECTION, errorWords, valueOf, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import type { Deriver } from "../data/host.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import type { NodeRow } from "./why.ts";

interface RowsArgs {
  deriver: Deriver;
  journey: string;
  query: ListQuery;
}

const ROWS: ReadSource<RowsArgs, NodeRow[]> = {
  policy: PROJECTION,
  read: async ({ deriver, journey, query }) => {
    const rows: NodeRow[] = [];
    let cursor: number | undefined = undefined;
    do {
      const page: Schema<"ListPage"> = await deriver.project(journey, { projection: "list", query: cursor === undefined ? query : { ...query, cursor } });
      rows.push(...page.rows);
      cursor = page.next ?? undefined;
    } while (cursor !== undefined);
    return rows;
  },
};

/** The key of a list's rows: the same read for the same journey and query, read again when the derivation moves. */
export function rowsKey(deriver: Deriver, view: Ready, query: ListQuery): ReadKey<RowsArgs> {
  const journey = view.key.journey;
  return { of: [journey, JSON.stringify(query)], freshness: [JSON.stringify(view.key)], args: { deriver, journey, query } };
}

export function useRows(view: Ready | undefined, query: ListQuery): { rows: NodeRow[] | undefined; error: string | undefined } {
  const { deriver } = useSession();
  const { view: answer } = useKeyedRead(ROWS, view === undefined ? undefined : rowsKey(deriver, view, query));
  return { rows: valueOf(answer), error: errorWords(answer) };
}
