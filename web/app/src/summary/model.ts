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
import { ownedBy } from "../acting/yours.ts";
import { guardFailureText, namer } from "../detail/explain.ts";
import { feedingDecision, nodeOf, recordOf, titleOf, type GraphNode, type Ready } from "../detail/model.ts";
import { entityName } from "../detail/sections.tsx";
import { daysWords } from "../status/when.ts";
import { DISPLAY_STATES, stateWord, type DisplayState } from "../status/words.ts";
import { treeOf } from "../plan/tree.ts";
import { dateWords, dayOf, type DateOrigin } from "../timeline/model.ts";

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
  /** Milestones not yet reached with their effective dates (F1), earliest first. */
  upcoming: (Listed & { date: string; origin: DateOrigin; owners: string[] })[];
  /** Open decisions with their owners by name, in rank order; no owner is unassigned. */
  openDecisions: (Listed & { owners: string[] })[];
}

function listed(ready: Ready, key: string): Listed {
  return { key, title: titleOf(ready, key) };
}

/** A node's owners by name, from its effective participations (E2). */
export function ownersOf(ready: Ready, key: string): string[] {
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
      owners: (each.owners ?? []).map((owner) => entityName(ready, owner)),
    })),
  };
}

/** Whether `node` counts in the journey's scope: relevant or undecided, and not skipped through a container (D1a). */
function inScope(ready: Ready, key: string): boolean {
  const derived = ready.derived.nodes[key];
  return derived !== undefined && derived.relevance.value !== "not_relevant" && derived.effectively_skipped !== true;
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
  /** Whether it is reached: by an actual date, or by `auto_reach` once its date has passed (F1). */
  reached: boolean;
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
  const final = (ready.journey.graph.nodes ?? []).find((node) => node.final === true && inScope(ready, node.key));
  if (final !== undefined) {
    const dates = ready.derived.nodes[final.key]?.dates;
    const feeder = dates?.effective_date === undefined ? feedingDecision(ready, final.key) : undefined;
    return {
      role: "final",
      key: final.key,
      title: final.title,
      date: dates?.effective_date?.date,
      origin: dates?.effective_date?.origin,
      reached: dates?.effective_date?.origin === "actual" || ready.derived.nodes[final.key]?.auto_reached === true,
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
    reached: next.origin === "actual",
    shortfallDays: ready.derived.nodes[next.key]?.dates.shortfall?.shortfall_days,
    feeder: undefined,
  };
}

/**
 * F1: where a milestone's date stands in words, the nearness first and the calendar date after it:
 * `in 18 days (Nov 23)`, `3 days late (Oct 2)`, `due today (Oct 6)`; an actual date is `reached Nov 20`
 * (never late).
 */
export function whenWords(date: string, origin: DateOrigin | undefined, today: string): string {
  if (origin === "actual") {
    return `reached ${dateWords(date, today)}`;
  }
  const away = dayOf(date) - dayOf(today);
  const when = dateWords(date, today);
  if (away < 0) {
    return `${daysWords(-away)} late (${when})`;
  }
  return away === 0 ? `due today (${when})` : `in ${daysWords(away)} (${when})`;
}

/** The words for where a date comes from; an actual date says it by `whenWords`. */
export const ORIGIN_WORDS: Record<DateOrigin, string | undefined> = { actual: undefined, pin: "pinned", due: "derived" };

/** A node that needs a look, with every reason, worst first (C18). */
export interface Attention {
  key: string;
  /** Every reason in words, worst first: `3 days late`, `2 days short`, `stale: missing artifact`. */
  reasons: string[];
  tone: FactTone;
}

/** `Reached 2 days off what its chain allows`, or `Needs 2 days more than its chain allows`: what a shortfall of `count` days means for node `key`. */
export function shortfallWords(ready: Ready, key: string, count: number | undefined): string {
  if (count === undefined) {
    return "Cannot be met as planned";
  }
  const node = nodeOf(ready, key);
  const state = node === undefined ? undefined : recordOf(ready, node).state;
  // A recorded date can break a chain by being too late or too early, so a finished node's shortfall says only how far off it is.
  if (state === "reached") {
    return `Reached ${daysWords(count)} off what its chain allows`;
  }
  return state === "done" || state === "decided" ? `Finished ${daysWords(count)} off what its chain allows` : `Needs ${daysWords(count)} more than its chain allows`;
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

/** What a waiting decision waits on, in words: that it may not apply, a snooze's date, or its first unblocker, milestones first. */
export function waitingFact(ready: Ready, node: GraphNode): string | undefined {
  if (ready.derived.nodes[node.key]?.display_state === "conditional") {
    return "May not apply";
  }
  const { unblockers, snoozedUntil } = unblockersOf(ready, node);
  const [first, ...rest] = [...unblockers].sort((left, right) => Number(right.kind === "milestone") - Number(left.kind === "milestone") || left.node.localeCompare(right.node));
  if (first === undefined) {
    return snoozedUntil === undefined ? undefined : `Snoozed until ${dateWords(snoozedUntil, ready.derived.today)}`;
  }
  return `Waiting for ${titleOf(ready, first.node)}${rest.length === 0 ? "" : ` and ${String(rest.length)} more`}`;
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
export function claimActions(ready: Ready, model: SummaryModel, frontier: readonly NodeRow[], viewerEntities: readonly string[]): Actions {
  const yours: NodeRow[] = [];
  const decide: NodeRow[] = [];
  const nextUp: NodeRow[] = [];
  for (const row of frontier) {
    if (ownedBy(ready, row.owners, viewerEntities)) {
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
  /** `Done Oct 3`, `Decided Oct 2`, `Reached Oct 1`. */
  fact: string;
}

const FINISHED_WORDS: Partial<Record<string, string>> = { done: "Done", decided: "Decided", reached: "Reached" };

/** How many finished nodes the page lists. */
export const RECENT_SHOWN = 5;

/**
 * F2, C18: the in-scope nodes whose stored state is finished and which recorded a finish date,
 * newest first, then by key, at most `RECENT_SHOWN`. A reopened or skipped node, one skipped
 * through a container, a node out of scope, and a milestone reached by its date alone (no
 * record) are left out.
 */
export function recentlyFinished(ready: Ready): Finished[] {
  const finished = (ready.journey.graph.nodes ?? []).flatMap((node) => {
    const record = recordOf(ready, node);
    const word = FINISHED_WORDS[record.state];
    return word === undefined || record.finished_on == null || !inScope(ready, node.key) ? [] : [{ key: node.key, on: record.finished_on, word }];
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

/** How many of the nodes inside container `key` are not finished yet, and how many count at all (D8). */
function insideOf(ready: Ready, key: string): { open: number; total: number } {
  const children = treeOf(ready).children;
  let open = 0;
  let total = 0;
  const walk = (parent: string) => {
    for (const child of children.get(parent) ?? []) {
      if ((children.get(child) ?? []).length > 0) {
        walk(child);
        continue;
      }
      const state = ready.derived.nodes[child]?.display_state;
      if (state !== undefined && state !== "not_relevant" && state !== "skipped") {
        total += 1;
        open += state === "done" ? 0 : 1;
      }
    }
  };
  walk(key);
  return { open, total };
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
        const { open, total } = insideOf(ready, node.key);
        return total === 0 ? undefined : `${String(open)} of ${String(total)} inside still open`;
      },
    }));
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
  const unfinished = left === 0 ? "" : ` with ${String(left)} ${left === 1 ? "node" : "nodes"} left unfinished`;
  return status === "completed" ? `Completed${unfinished}.` : `Archived and read-only${unfinished}.`;
}
