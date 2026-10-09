// C9: every row the list's query matches, read a page after another from the tab's derivation
// (the engine answers `page_item_count_max` rows at a time) until there are no more. The tree
// needs them all to put each match under its container. Like the canvas's reads it keeps the
// last answer for the same request until the next arrives, so a live edit swaps the rows once.
import type { Schema } from "@cairn/client";
import type { ListQuery } from "@cairn/wasm";
import { useEffect, useState } from "react";

import { heldFor, type Answered } from "../canvas/hooks.ts";
import { useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import type { NodeRow } from "./why.ts";

export function useRows(view: Ready, query: ListQuery): { rows: NodeRow[] | undefined; error: string | undefined } {
  const { deriver } = useSession();
  const journey = view.key.journey;
  const requested = JSON.stringify([journey, query]);
  const asked = JSON.stringify([view.key, query]);
  const [answered, setAnswered] = useState<Answered<NodeRow[]> | undefined>(undefined);
  useEffect(() => {
    let live = true;
    const read = async () => {
      const rows: NodeRow[] = [];
      let cursor: number | undefined = undefined;
      do {
        const page: Schema<"ListPage"> = await deriver.project(journey, { projection: "list", query: cursor === undefined ? query : { ...query, cursor } });
        rows.push(...page.rows);
        cursor = page.next ?? undefined;
      } while (cursor !== undefined);
      return rows;
    };
    read().then(
      (rows) => {
        if (live) {
          setAnswered({ request: requested, value: rows });
        }
      },
      (thrown: unknown) => {
        if (live) {
          setAnswered({ request: requested, error: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
    // `asked` names the request and the derivation it is read from, so it alone is the key.
  }, [deriver, journey, asked]);
  const { value, error } = heldFor(answered, requested);
  return { rows: value, error };
}
