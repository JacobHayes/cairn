// The figures of a journey in the index, as data (4.8): progress from the status summary, the one
// flag, and the summary read for a derived journey. Pure but for the one hook.
import { useMemo } from "react";

import { useProjected } from "../canvas/hooks.ts";
import type { Ready } from "../detail/model.ts";
import { summaryModel, type SummaryModel } from "../summary/model.ts";

/** The journey's status summary, read from its local derivation. */
export function useStatusSummary(ready: Ready): SummaryModel | undefined {
  const summary = useProjected(ready, { projection: "status_summary" });
  return useMemo(() => (summary.value === undefined ? undefined : summaryModel(ready, summary.value)), [ready, summary.value]);
}

/** Done over in scope: what is in scope less what has work left on it. */
export function progressOf(model: SummaryModel): { done: number; inScope: number } {
  return { done: model.inScope - model.remaining, inScope: model.inScope };
}

/** The one flag a journey carries in the index: stalled, else what is overdue, else what is stale, with `+N` for the rest. */
export function flagOf(ready: Ready, model: SummaryModel | undefined): string {
  if (ready.derived.stalled != null) {
    return "STALLED";
  }
  if (model === undefined) {
    return "";
  }
  const flags = [
    model.overdue.length > 0 ? `${String(model.overdue.length)} OVERDUE` : undefined,
    model.stale.length > 0 ? `${String(model.stale.length)} STALE` : undefined,
    model.shortfalls.length > 0 ? `${String(model.shortfalls.length)} SHORT` : undefined,
  ].filter((each) => each !== undefined);
  const [first, ...rest] = flags;
  return first === undefined ? "" : rest.length === 0 ? first : `${first} +${String(rest.length)}`;
}
