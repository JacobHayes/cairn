// B6: what a snooze can set aside and until when (design 6.5): the node itself or an ancestor
// container holding unfinished work, each with its open count, nearest first; and the quick
// dates. Pure, so the unit tests check them without a page.
import type { Schema } from "@cairn/client";

import { dateOf, dayOf } from "../timeline/model.ts";
import { nodeOf, type NodeDetail, type Ready } from "./model.ts";

/** Something a snooze can set aside. */
export interface SetAside {
  node: string;
  title: string;
  /** For a container: the unfinished in-scope items beneath it (the node itself has none). */
  open: number | undefined;
}

/** Whether `key` lies beneath `ancestor`. */
function beneath(view: Ready, key: string, ancestor: string): boolean {
  let parent = nodeOf(view, key)?.parent ?? undefined;
  const seen = new Set<string>();
  while (parent !== undefined && !seen.has(parent)) {
    if (parent === ancestor) {
      return true;
    }
    seen.add(parent);
    parent = nodeOf(view, parent)?.parent ?? undefined;
  }
  return false;
}

/** The unfinished, in-scope work beneath a container: what its snooze would hold back. */
export function openUnder(view: Ready, container: string): number {
  const progress = view.derived.nodes[container]?.progress;
  return progress == null ? 0 : progress.total - progress.done;
}

/** The node, then each ancestor container the engine reads as snoozable, nearest first, with its open count. */
export function setAsideOptions(view: Ready, detail: NodeDetail): SetAside[] {
  const own: SetAside[] = detail.derived.snoozable === true ? [{ node: detail.node.key, title: detail.node.title, open: undefined }] : [];
  const ancestors = [...detail.ancestors].reverse().flatMap((ancestor): SetAside[] => (view.derived.nodes[ancestor.key]?.snoozable === true ? [{ node: ancestor.key, title: ancestor.title, open: openUnder(view, ancestor.key) }] : []));
  return [...own, ...ancestors];
}

/** The day after `today`. */
export function tomorrow(today: string): string {
  return dateOf(dayOf(today) + 1);
}

/** The next Monday after `today` (a Monday asks for the one a week on). */
export function nextMonday(today: string): string {
  const day = dayOf(today);
  // 1970-01-01 was a Thursday, so day 4 is the first Monday.
  const toMonday = (((4 - day) % 7) + 7) % 7;
  return dateOf(day + (toMonday === 0 ? 7 : toMonday));
}

/** Everything at or beneath `key`: the nodes a snooze of it must not wait on (B6). */
export function withinScope(view: Ready, key: string): Set<string> {
  return new Set([key, ...(view.journey.graph.nodes ?? []).filter((node) => beneath(view, node.key, key)).map((node) => node.key)]);
}

export const UNTIL_OPTIONS = [
  { value: "tomorrow", label: "Tomorrow" },
  { value: "monday", label: "Next Monday" },
  { value: "date", label: "A date" },
  { value: "node", label: "When something is done" },
] as const;

/** What a snooze waits for: a quick date, a chosen date, or another node being done. */
export type Until = (typeof UNTIL_OPTIONS)[number]["value"];

/** B6: what the snooze form holds: what to set aside and until when, kept across a reload. */
export interface SnoozeDraft {
  aside: string;
  until: Until;
  date: string;
  target: string;
}

/** The snooze form opened on the node itself, until tomorrow. */
export function snoozeStart(view: Ready, node: string): SnoozeDraft {
  return { aside: node, until: "tomorrow", date: tomorrow(view.derived.today), target: "" };
}

/** What the draft waits for, or none while it names no date or node yet. */
export function untilOf(draft: SnoozeDraft, today: string): Schema<"SnoozeTarget"> | undefined {
  switch (draft.until) {
    case "tomorrow":
      return { date: tomorrow(today) };
    case "monday":
      return { date: nextMonday(today) };
    case "date":
      return draft.date === "" ? undefined : { date: draft.date };
    case "node":
      return draft.target === "" ? undefined : { node: draft.target };
  }
}
