// What a list of journeys (Home, the journey index) reads of each one: the viewer's work in it,
// what of that can move now, and, for a table row, the status summary. Each is the journey's own
// local derivation (H6), projected in the derive worker, and a reading is one value: still being
// read, gone, failed (with the words, so the list can say so and offer to try again), or ready.
// An unknown is never shown as zero.
import type { NextQuery } from "@cairn/wasm";
import { useMemo } from "react";

import type { NodeRow } from "../acting/why.ts";
import { useProjected } from "../canvas/hooks.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import type { Ready } from "../detail/model.ts";
import { summaryModel, type SummaryModel } from "../summary/model.ts";
import { useMineOf, type MineEntry } from "./mine.ts";

/** The viewer's own work that can be done now, in the journey's rank (E4, C10). */
export const READY_FOR_VIEWER: NextQuery = { sort: "rank", mine: true, kinds: [] };

const READY_REQUEST = { projection: "next" as const, query: READY_FOR_VIEWER };
const SUMMARY_REQUEST = { projection: "status_summary" as const };

export type JourneyReading =
  | { status: "loading" }
  | { status: "gone" }
  | { status: "failed"; message: string }
  | {
      status: "ready";
      view: Ready;
      /** Everything of the viewer's in the journey, unranked. */
      mine: MineEntry[];
      /** The viewer's work that can move now, in the journey's rank. */
      ready: NodeRow[];
      /** The status summary, when it was asked for. */
      model: SummaryModel | undefined;
    };

/** Journey `id` as a list reads it; with `summary`, its status summary too. */
export function useJourneyReading(id: string, summary: boolean): JourneyReading {
  const mine = useMineOf(id);
  const view = mine.status === "ready" ? mine.view : undefined;
  const entries = mine.status === "ready" ? mine.entries : undefined;
  const next = useProjected(view, view === undefined ? undefined : READY_REQUEST);
  const status = useProjected(view, view === undefined || !summary ? undefined : SUMMARY_REQUEST);
  const rows = valueOf(next)?.items;
  const found = valueOf(status);
  const failed = mine.status === "failed" ? mine.message : (errorWords(next) ?? errorWords(status));
  return useMemo((): JourneyReading => {
    if (mine.status === "gone") {
      return { status: "gone" };
    }
    if (failed !== undefined) {
      return { status: "failed", message: failed };
    }
    if (view === undefined || entries === undefined || rows === undefined || (summary && found === undefined)) {
      return { status: "loading" };
    }
    return { status: "ready", view, mine: entries, ready: rows, model: found === undefined ? undefined : summaryModel(view, found) };
  }, [mine.status, failed, view, entries, rows, found, summary]);
}
