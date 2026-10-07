// E4, C16: one journey's "mine", projected in the derive worker over the journey the tab
// holds derived (and keeps current, H6), with the derive inputs' viewer: the nodes where the
// caller's entities hold a participation, with the kinds they hold. The cross-journey "mine"
// list and the index's "mine" filter are both the union of these, unranked.
import type { Schema } from "@cairn/client";
import { useCallback, useEffect, useState } from "react";

import type { JourneyView } from "../data/journeys.ts";
import { useJourney, useSession } from "../data/react.ts";

export type MineEntry = Schema<"MineEntry">;

type Ready = Extract<JourneyView, { status: "ready" }>;

export type MineView =
  | { status: "loading" }
  | { status: "gone" }
  | { status: "failed"; message: string }
  | { status: "ready"; view: Ready; entries: MineEntry[] };

/** Journey `id`'s "mine", re-projected whenever the journey is derived again. */
export function useMineOf(id: string): MineView {
  const { deriver } = useSession();
  const journey = useJourney(id);
  const [answer, setAnswer] = useState<{ key: Ready["key"]; entries: MineEntry[] } | { error: string } | undefined>();
  const ready = journey.status === "ready" ? journey : undefined;
  useEffect(() => {
    if (ready === undefined) {
      return;
    }
    let live = true;
    deriver.project(id, { projection: "mine" }).then(
      (entries) => {
        if (live) {
          setAnswer({ key: ready.key, entries });
        }
      },
      (thrown: unknown) => {
        if (live) {
          setAnswer({ error: thrown instanceof Error ? thrown.message : String(thrown) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [deriver, id, ready]);
  if (journey.status === "missing") {
    return { status: "gone" };
  }
  if (journey.status === "failed") {
    return { status: "failed", message: journey.message };
  }
  if (answer !== undefined && "error" in answer) {
    return { status: "failed", message: answer.error };
  }
  if (ready === undefined || answer?.key !== ready.key) {
    return { status: "loading" };
  }
  return { status: "ready", view: ready, entries: answer.entries };
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
