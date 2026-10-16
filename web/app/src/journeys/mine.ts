// E4, C16: one journey's "mine", projected in the derive worker over the journey the tab
// holds derived (and keeps current, H6), with the derive inputs' viewer: the nodes where the
// caller's entities hold a participation, with the kinds they hold. The cross-journey "mine"
// list and the index's "mine" filter are both the union of these, unranked.
import type { Schema } from "@cairn/client";
import { useCallback, useEffect, useState } from "react";

import { useProjected } from "../canvas/hooks.ts";
import type { JourneyView } from "../data/journeys.ts";
import { useJourney } from "../data/react.ts";

export type MineEntry = Schema<"MineEntry">;

type Ready = Extract<JourneyView, { status: "ready" }>;

export type MineView =
  | { status: "loading" }
  | { status: "gone" }
  | { status: "failed"; message: string }
  | { status: "ready"; view: Ready; entries: MineEntry[] };

/** Journey `id`'s "mine", re-projected whenever the journey is derived again. */
export function useMineOf(id: string): MineView {
  const journey = useJourney(id);
  const ready = journey.status === "ready" ? journey : undefined;
  const mine = useProjected(ready, ready === undefined ? undefined : { projection: "mine" });
  if (journey.status === "missing") {
    return { status: "gone" };
  }
  if (journey.status === "failed") {
    return { status: "failed", message: journey.message };
  }
  if (mine.status === "failed" || mine.status === "behind") {
    return { status: "failed", message: mine.error.message };
  }
  if (ready === undefined || mine.status !== "current") {
    return { status: "loading" };
  }
  return { status: "ready", view: ready, entries: mine.value };
}

/**
 * How many of `items` hold something of the caller's, as each journey's "mine" settles: a
 * list filtered to "mine" says so when none does, rather than showing an empty table.
 */
export function useMineCounts(items: readonly { id: string }[]) {
  const [counts, setCounts] = useState<Record<string, number>>({});
  const report = useCallback((id: string, count: number) => {
    setCounts((held) => (held[id] === count ? held : { ...held, [id]: count }));
  }, []);
  const settled = items.every((item) => counts[item.id] !== undefined);
  const total = items.reduce((sum, item) => sum + (counts[item.id] ?? 0), 0);
  return { report, none: settled && total === 0 };
}

/**
 * Reports journey `id`'s "mine" count to `report` once it settles: none for one gone, and
 * nothing for one that could not be read, which stays unknown (it may hold the caller's work).
 */
export function useReportMine(id: string, mine: MineView, report: (id: string, count: number) => void): void {
  const count = mine.status === "ready" ? mine.entries.length : mine.status === "gone" ? 0 : undefined;
  useEffect(() => {
    if (count !== undefined) {
      report(id, count);
    }
  }, [id, count, report]);
}
