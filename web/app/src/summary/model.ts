// C18: the journey status summary as data, for observers and reporting. The engine's projection
// counts the in-scope nodes by state and lists what remains, the overdue, short, and stale
// nodes, the upcoming milestones with their effective dates, and the open decisions with their
// owners in rank order; this adds the titles, names, dates, and reasons a reader needs. Pure,
// so the unit tests check it without a page.
//
// Cost: O(in-scope nodes) per summary, with a map from key to node.
import type { Schema } from "@cairn/client";

import { guardFailureText, namer } from "../detail/explain.ts";
import { titleOf, type Ready, type State } from "../detail/model.ts";
import { entityName } from "../detail/sections.tsx";
import { dayOf, type DateOrigin } from "../timeline/model.ts";

export type StatusSummary = Schema<"StatusSummary">;

/** The states in the order a summary lists them: work, decisions, milestones, then groups. */
export const STATE_ORDER: State[] = ["todo", "active", "done", "skipped", "open", "decided", "pending", "reached", "derived"];

/** A state as a reader reads it in a count. */
const STATE_WORDS: Record<State, string> = {
  todo: "to do",
  active: "active",
  done: "done",
  skipped: "skipped",
  open: "open decisions",
  decided: "decided",
  pending: "pending milestones",
  reached: "reached",
  derived: "groups",
};

/** One count by state. */
export interface StateCount {
  state: State;
  words: string;
  count: number;
}

/** A node the summary lists, with what it says about it. */
export interface Listed {
  key: string;
  title: string;
}

/** C18: the summary a reader sees. */
export interface SummaryModel {
  /** In-scope nodes by state, in `STATE_ORDER`, the states with none left out. */
  byState: StateCount[];
  inScope: number;
  remaining: number;
  /** Overdue nodes with their due dates and how many days late they are. */
  overdue: (Listed & { due: string | undefined; lateDays: number | undefined })[];
  /** Nodes with a shortfall and the days they are short (F6). */
  shortfalls: (Listed & { days: number | undefined })[];
  /** Stale nodes with why each is stale (D4). */
  stale: (Listed & { reasons: string[] })[];
  /** Milestones not yet reached with their effective dates (F1), earliest first. */
  upcoming: (Listed & { date: string; origin: DateOrigin; owners: string[] })[];
  /** Open decisions with their owners by name, in rank order; no owner is unassigned. */
  openDecisions: (Listed & { owners: string[] })[];
}

function listed(ready: Ready, key: string): Listed {
  return { key, title: titleOf(ready, key) };
}

/** A node's owners by name, from its effective participations (E2). */
function ownersOf(ready: Ready, key: string): string[] {
  return (ready.derived.nodes[key]?.participations?.["k_owner"]?.entities ?? []).map((entity) => entityName(ready, entity));
}

function overdueOf(ready: Ready, key: string): SummaryModel["overdue"][number] {
  const due = ready.derived.nodes[key]?.dates.due?.date;
  return { ...listed(ready, key), due, lateDays: due === undefined ? undefined : dayOf(ready.derived.today) - dayOf(due) };
}

function staleOf(ready: Ready, key: string): SummaryModel["stale"][number] {
  const name = namer(ready);
  return { ...listed(ready, key), reasons: (ready.derived.nodes[key]?.stale ?? []).map((failure) => guardFailureText(failure, name)) };
}

/** C18: the projected summary with what a reader needs to read it. */
export function summaryModel(ready: Ready, summary: StatusSummary): SummaryModel {
  const count = (state: State) => summary.by_state[state] ?? 0;
  return {
    byState: STATE_ORDER.filter((state) => count(state) > 0).map((state) => ({ state, words: STATE_WORDS[state], count: count(state) })),
    inScope: Object.values(summary.by_state).reduce((sum, each) => sum + each, 0),
    remaining: summary.remaining,
    overdue: (summary.overdue ?? []).map((key) => overdueOf(ready, key)),
    shortfalls: (summary.shortfalls ?? []).map((key) => ({ ...listed(ready, key), days: ready.derived.nodes[key]?.dates.shortfall?.shortfall_days })),
    stale: (summary.stale ?? []).map((key) => staleOf(ready, key)),
    upcoming: (summary.upcoming_milestones ?? []).map((each) => ({
      ...listed(ready, each.node),
      date: each.date.date,
      origin: each.date.origin,
      owners: ownersOf(ready, each.node),
    })),
    openDecisions: (summary.open_decisions ?? []).map((each) => ({
      ...listed(ready, each.node),
      owners: (each.owners ?? []).map((owner) => entityName(ready, owner)),
    })),
  };
}
