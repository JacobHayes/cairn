// C18: the journey status summary as data, for observers and reporting. The engine's projection
// counts the in-scope nodes by display state (D8) and lists what remains, the overdue, short, and stale
// nodes, the upcoming milestones with their effective dates, and the open decisions with their
// owners in rank order; this adds the titles, names, dates, and reasons a reader needs. Pure,
// so the unit tests check it without a page.
//
// Cost: O(in-scope nodes) per summary, with a map from key to node. The page's own functions
// below (the final milestone, what needs attention, who has which action, what finished
// recently) are each one pass over a projection's rows or the nodes, plus a sort.
import type { Schema } from "@cairn/client";

import { unblockersOf } from "../acting/waiting.ts";
import type { FactTone } from "../acting/reasons.ts";
import type { NodeRow } from "../acting/why.ts";
import { guardFailureText, namer } from "../detail/explain.ts";
import { feedingDecision, itemTitle, milestonesFirst, nodeOf, recordOf, scheduledDate, titleOf, type GraphNode, type Ready } from "../detail/model.ts";
import { snoozeEndWords } from "../status/snooze.ts";
import { isOpen } from "../status/state.ts";
import { dateWords, daysWords } from "../status/when.ts";
import { DISPLAY_STATES, stateWord, type DisplayState } from "../status/words.ts";
import { treeOf } from "../plan/tree.ts";
import type { DateOrigin } from "../timeline/model.ts";
import { plural } from "../ui/words.ts";
import { ownersOf } from "../detail/sections.tsx";

export type StatusSummary = Schema<"StatusSummary">;

/** One count by display state. */
export interface StateCount {
  state: DisplayState;
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
  /** In-scope nodes by display state, the states with none left out. */
  byState: StateCount[];
  inScope: number;
  remaining: number;
  /** Overdue nodes with their due dates and how many days late they are. */
  overdue: (Listed & { due: string | undefined; lateDays: number | undefined })[];
  /** Nodes with a shortfall and the days they are short (F6). */
  shortfalls: (Listed & { days: number | undefined })[];
  /** Stale nodes with why each is stale (D4). */
  stale: (Listed & { reasons: string[] })[];
  /** Milestones not yet done with their effective dates (F1), earliest first. */
  upcoming: (Listed & { date: string; origin: DateOrigin; owners: string[] })[];
  /** Open decisions with their owners (identity keys), in rank order; no owner is unassigned. */
  openDecisions: (Listed & { owners: string[] })[];
}

function listed(ready: Ready, key: string): Listed {
  return { key, title: titleOf(ready, key) };
}

function overdueOf(ready: Ready, key: string): SummaryModel["overdue"][number] {
  const dates = ready.derived.nodes[key]?.dates;
  return { ...listed(ready, key), due: dates?.due?.date, lateDays: dates?.overdue_days ?? undefined };
}

function staleOf(ready: Ready, key: string): SummaryModel["stale"][number] {
  const name = namer(ready);
  return { ...listed(ready, key), reasons: (ready.derived.nodes[key]?.stale ?? []).map((failure) => guardFailureText(failure, name, ready.derived.today, (id) => itemTitle(ready, key, id))) };
}

/** C18: the projected summary with what a reader needs to read it. */
export function summaryModel(ready: Ready, summary: StatusSummary): SummaryModel {
  const count = (state: DisplayState) => summary.by_display_state[state] ?? 0;
  return {
    byState: DISPLAY_STATES.filter((state) => count(state) > 0).map((state) => ({ state, words: stateWord(state), count: count(state) })),
    inScope: Object.values(summary.by_display_state).reduce((sum, each) => sum + each, 0),
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
      owners: each.owners ?? [],
    })),
  };
}

/** The milestone the lead names: the journey's final one, or with none the next one (C13). */
export interface MilestoneLine {
  /** `final` is the end (A1a); `next` is only the earliest milestone ahead. */
  role: "final" | "next";
  key: string;
  title: string;
  /** Its effective date (F1) and where it comes from; none while nothing gives it one. */
  date: string | undefined;
  origin: DateOrigin | undefined;
  /** Whether it is done, as its display state says: by an actual date, or by `auto_done` once its date has passed (F1). */
  done: boolean;
  /** Its own shortfall in days (F6), never another node's. */
  shortfallDays: number | undefined;
  /** The decision whose answer would pin an undated final milestone (E3). */
  feeder: { key: string; title: string } | undefined;
}

/**
 * C13, C18, F1, F6, E3: the lead's milestone line. Only the `final` milestone is ever the end;
 * without one (or with the final one out of scope) the earliest upcoming milestone is named as
 * the next, and with none the line is left out.
 */
export function milestoneLine(ready: Ready, model: SummaryModel): MilestoneLine | undefined {
  // In scope and not skipped (D1a): still to do, or finished.
  const final = (ready.journey.graph.nodes ?? []).find((node) => {
    const derived = ready.derived.nodes[node.key];
    return node.final === true && (isOpen(derived) || derived?.display_state === "done");
  });
  if (final !== undefined) {
    const dates = ready.derived.nodes[final.key]?.dates;
    const feeder = dates?.effective_date === undefined ? feedingDecision(ready, final.key) : undefined;
    return {
      role: "final",
      key: final.key,
      title: final.title,
      date: dates?.effective_date?.date,
      origin: dates?.effective_date?.origin,
      done: ready.derived.nodes[final.key]?.display_state === "done",
      shortfallDays: dates?.shortfall?.shortfall_days,
      feeder: feeder === undefined ? undefined : { key: feeder.key, title: feeder.title },
    };
  }
  const next = model.upcoming[0];
  if (next === undefined) {
    return undefined;
  }
  return {
    role: "next",
    key: next.key,
    title: next.title,
    date: next.date,
    origin: next.origin,
    done: ready.derived.nodes[next.key]?.display_state === "done",
    shortfallDays: ready.derived.nodes[next.key]?.dates.shortfall?.shortfall_days,
    feeder: undefined,
  };
}

/** A node that needs a look, with every reason, worst first (C18). */
export interface Attention {
  key: string;
  /** Every reason in words, worst first: `3 days late`, `2 days short`, `stale: missing artifact`. */
  reasons: string[];
  tone: FactTone;
}

/** `Finished 2 days off what its chain allows`, or `Needs 2 days more than its chain allows`: what a shortfall of `count` days means for node `key`. */
export function shortfallWords(ready: Ready, key: string, count: number | undefined): string {
  if (count === undefined) {
    return "Cannot be met as planned";
  }
  const finished = ready.derived.nodes[key]?.display_state === "done";
  // A recorded date can break a chain by being too late or too early, so a finished node's shortfall says only how far off it is.
  return finished ? `Finished ${daysWords(count)} off what its chain allows` : `Needs ${daysWords(count)} more than its chain allows`;
}

/**
 * C18, D3, D4, F6: every overdue, short, or stale node once, worst first (overdue, then short,
 * then stale), then by key, each with all of its reasons in that order, in plain words.
 */
export function attentionRows(ready: Ready, model: SummaryModel): Attention[] {
  const found = new Map<string, { rank: number; reasons: string[] }>();
  const add = (key: string, rank: number, reason: string) => {
    const held = found.get(key) ?? { rank, reasons: [] };
    held.reasons.push(reason);
    found.set(key, held);
  };
  for (const each of model.overdue) {
    add(each.key, 0, each.lateDays === undefined ? "Overdue" : `${daysWords(each.lateDays)} late`);
  }
  for (const each of model.shortfalls) {
    add(each.key, 1, shortfallWords(ready, each.key, each.days));
  }
  for (const each of model.stale) {
    add(each.key, 2, each.reasons.length === 0 ? "Stale" : `Stale: ${each.reasons.join(", ")}`);
  }
  return [...found.entries()]
    .sort(([left, a], [right, b]) => a.rank - b.rank || left.localeCompare(right))
    .map(([key, held]) => ({ key, reasons: held.reasons, tone: held.rank === 2 ? "warn" : "bad" }));
}

/** An open decision that cannot be made yet, and what it waits on in words. */
export interface WaitingDecision {
  key: string;
  owners: string[];
  /** `Waiting for Test plan`, `Snoozed until Oct 20`, or `May not apply`; none when nothing is known. */
  fact: string | undefined;
}

/** D8: the date a scheduled node waits for, in words: `Not before Nov 1`, or a milestone's `On Nov 1`; none for one no date holds. */
export function scheduledFact(ready: Ready, node: GraphNode): string | undefined {
  const date = scheduledDate(ready, node.key);
  return date === undefined ? undefined : `${node.kind === "milestone" ? "On" : "Not before"} ${dateWords(date, ready.derived.today)}`;
}

/** What a waiting decision waits on, in words: that it may not apply, its own date, a snooze's date, or its first unblocker, milestones first, with the date it is scheduled for. */
export function waitingFact(ready: Ready, node: GraphNode): string | undefined {
  if (ready.derived.nodes[node.key]?.display_state === "conditional") {
    return "May not apply";
  }
  const scheduled = scheduledFact(ready, node);
  if (scheduled !== undefined) {
    return scheduled;
  }
  const { unblockers, snoozedUntil } = unblockersOf(ready, node);
  const [first, ...rest] = milestonesFirst(ready, unblockers, (each) => each.node);
  if (first === undefined) {
    return snoozedUntil === undefined ? undefined : `Snoozed until ${snoozeEndWords(ready, { date: snoozedUntil })}`;
  }
  const on = first.date === undefined ? "" : ` on ${dateWords(first.date, ready.derived.today)}`;
  return `Waiting for ${titleOf(ready, first.node)}${on}${rest.length === 0 ? "" : ` and ${String(rest.length)} more`}`;
}

/** The sets of action rows the page lists, each node in one (C10, C11, E1, E4). */
export interface Actions {
  /** The acting-frontier items the viewer owns, in rank order. */
  yours: NodeRow[];
  /** The decisions on the acting frontier, in rank order, not the viewer's. */
  decide: NodeRow[];
  /** The rest of the acting frontier, in rank order. */
  nextUp: NodeRow[];
  /** Open in-scope decisions off the acting frontier, in the summary's rank order. */
  waiting: WaitingDecision[];
}

/**
 * C10, C11, E1, E4, C18: one action row per node. The viewer's owned items are claimed first,
 * then the decisions that can be made now, then the rest of the frontier, so a count is its
 * set's whole size whatever the page shows of it. A decision off the frontier waits.
 */
export function claimActions(ready: Ready, model: SummaryModel, frontier: readonly NodeRow[], owned: ReadonlySet<string>): Actions {
  const yours: NodeRow[] = [];
  const decide: NodeRow[] = [];
  const nextUp: NodeRow[] = [];
  for (const row of frontier) {
    if (owned.has(row.key)) {
      yours.push(row);
    } else if (row.kind === "decision") {
      decide.push(row);
    } else {
      nextUp.push(row);
    }
  }
  const acting = new Set(frontier.map((row) => row.key));
  const waiting = model.openDecisions
    .filter((each) => !acting.has(each.key))
    .flatMap((each) => {
      const node = nodeOf(ready, each.key);
      return node === undefined ? [] : [{ key: each.key, owners: each.owners, fact: waitingFact(ready, node) }];
    });
  return { yours, decide, nextUp, waiting };
}

/** A finished node and when it was recorded finished (F2). */
export interface Finished {
  key: string;
  /** `Done Oct 3`, `Decided Oct 2`. */
  fact: string;
}

const FINISHED_WORDS: Partial<Record<string, string>> = { done: "Done", decided: "Decided" };

/** How many finished nodes the page lists. */
export const RECENT_SHOWN = 5;

/**
 * F2, C18: the in-scope nodes whose stored state is finished and which recorded a finish date,
 * newest first, then by key, at most `RECENT_SHOWN`. A reopened or skipped node, one skipped
 * through a container, a node out of scope, and a milestone done by its date alone (no
 * record) are left out.
 */
export function recentlyFinished(ready: Ready): Finished[] {
  const finished = (ready.journey.graph.nodes ?? []).flatMap((node) => {
    const record = recordOf(ready, node);
    const word = FINISHED_WORDS[record.state];
    return word === undefined || record.finished_on == null || ready.derived.nodes[node.key]?.display_state !== "done" ? [] : [{ key: node.key, on: record.finished_on, word }];
  });
  return finished
    .sort((left, right) => right.on.localeCompare(left.on) || left.key.localeCompare(right.key))
    .slice(0, RECENT_SHOWN)
    .map((each) => ({ key: each.key, fact: `${each.word} ${dateWords(each.on, ready.derived.today)}` }));
}

/** C18: whether the journey has no node at all, as against one with nothing in scope. */
export function isEmpty(ready: Ready): boolean {
  return (ready.journey.graph.nodes ?? []).length === 0;
}

/** A node listed as under way or as blocked, who owns it, and what it says (read only when its row is drawn). */
export interface Held {
  key: string;
  owners: string[];
  fact: () => string | undefined;
}

/** Every node on one of the action lists, so no node is on two (C18). */
export function claimedKeys(actions: Actions): Set<string> {
  return new Set([...actions.yours, ...actions.decide, ...actions.nextUp].map((row) => row.key).concat(actions.waiting.map((each) => each.key)));
}

/**
 * C18, D8: the nodes that are active but not on an action list, in graph order: the work under
 * way that nothing is asked of. A container says how much inside it is still open.
 */
export function underWay(ready: Ready, claimed: ReadonlySet<string>): Held[] {
  return (ready.journey.graph.nodes ?? [])
    .filter((node) => ready.derived.nodes[node.key]?.display_state === "active" && !claimed.has(node.key))
    .map((node) => ({
      key: node.key,
      owners: ownersOf(ready, node.key),
      fact: () => {
        const progress = ready.derived.nodes[node.key]?.progress;
        return progress == null || progress.total === 0 ? undefined : `${String(progress.total - progress.done)} of ${String(progress.total)} inside still open`;
      },
    }));
}

/**
 * C18, D8: the work and decisions waiting for their own date (scheduled) and not already on an
 * action list, in graph order, each with that date. A milestone is left out: the dates panel
 * lists every milestone ahead with its date.
 */
export function scheduledRows(ready: Ready, claimed: ReadonlySet<string>): Held[] {
  return (ready.journey.graph.nodes ?? [])
    .filter((node) => node.kind !== "milestone" && ready.derived.nodes[node.key]?.display_state === "scheduled" && !claimed.has(node.key))
    .map((node) => ({ key: node.key, owners: ownersOf(ready, node.key), fact: () => scheduledFact(ready, node) }));
}

/**
 * C18, D8: the blocked work not already on an action list, in graph order, each saying what it
 * waits for (a container with nothing but its own contents to wait for says so).
 */
export function blockedRows(ready: Ready, claimed: ReadonlySet<string>): Held[] {
  const children = treeOf(ready).children;
  return (ready.journey.graph.nodes ?? [])
    .filter((node) => ready.derived.nodes[node.key]?.display_state === "blocked" && !claimed.has(node.key))
    .map((node) => ({
      key: node.key,
      owners: ownersOf(ready, node.key),
      fact: () => waitingFact(ready, node) ?? ((children.get(node.key) ?? []).length > 0 ? "Waiting for what is inside it" : "Waiting for earlier work"),
    }));
}

/** C18, A19, B11: what a completed or archived journey says it is, with what it left unfinished; none while it is active. */
export function lifecycleLine(ready: Ready, model: SummaryModel): string | undefined {
  const status = ready.journey.header.status;
  if (status === "active") {
    return undefined;
  }
  const left = model.remaining;
  const unfinished = left === 0 ? "" : ` with ${plural(left, "node")} left unfinished`;
  return status === "completed" ? `Completed${unfinished}.` : `Archived and read-only${unfinished}.`;
}
