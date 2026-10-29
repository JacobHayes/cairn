// What the inspector reads from the tab's derive beyond the node's own values (ARCHITECTURE,
// Web UI: the browser derives locally): the node's rank terms, from the next list it ranks in,
// and the dependents it would not yet free, from the explanations projection. Each is read on
// demand for the open node only, never for every node.
import type { Schema } from "@cairn/client";

import type { NodeRow } from "../acting/why.ts";
import { useProjected } from "../canvas/hooks.ts";
import { valueOf } from "../data/keyed-read.ts";
import type { Ready } from "./model.ts";

type HeldDependent = Schema<"HeldDependent">;

/** C10: the node's row of the next list: its rank and its terms. None while unread, or when it is not on the acting frontier. */
export function useRankRow(view: Ready, key: string): NodeRow | undefined {
  const onFrontier = view.derived.acting_frontier.includes(key);
  const next = valueOf(useProjected(view, onFrontier ? { projection: "next" } : undefined));
  return next?.items.find((row) => row.key === key);
}

/** C8: the direct dependents completing the node would not yet free, with what else each waits on; `total` counts them all. */
export function useStillWaiting(view: Ready, key: string, enabled: boolean): { held: HeldDependent[]; total: number } | undefined {
  const page = valueOf(useProjected(view, enabled ? { projection: "explanations", key, field: "still_waiting" } : undefined));
  return page === undefined ? undefined : { held: page.held ?? [], total: page.total };
}
