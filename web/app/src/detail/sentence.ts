// C8, D8: the one plain sentence the inspector says under its header, built in the tab from the
// node's display state and what its derive already holds: what its state means for this node
// and, where it helps, what to do about it. Pure, so the unit tests check the wording without a
// page; the pieces carry links and the Unsnooze action, which the component draws.
import type { Schema } from "@cairn/client";

import { rankParts, type NodeRow } from "../acting/why.ts";
import { rollups } from "../plan/tree.ts";
import { dateWords } from "../timeline/model.ts";
import { guardFailureText, namer } from "./explain.ts";
import { INITIAL_STATE, isTerminal, nodeOf, titleOf, unansweredOf, type NodeDetail, type NodeDerived, type Ready } from "./model.ts";
import { answerText, entityName } from "./sections.tsx";
import { conditionPhrase, daysFrom, plural, relativeDays } from "./words.ts";

/** A part of the sentence: text, a link to a node, or the Unsnooze action for the container whose snooze holds over the node. */
export type Piece = string | { node: string } | { unsnooze: string };

/** What the inspector knows of the node's rank: its place on the acting frontier and its terms, once read. */
export interface RankFacts {
  /** One-based place on the ranked acting frontier, none when it is not on it. */
  position: number | undefined;
  row: NodeRow | undefined;
}

/** The place of `key` on the ranked acting frontier, one-based. */
export function positionOf(view: Ready, key: string): number | undefined {
  const at = view.derived.acting_frontier.indexOf(key);
  return at === -1 ? undefined : at + 1;
}

/** A dependency the node waits on: what it is, how it arose, and the ancestor holding it for the node, if one does. */
export interface Holding {
  node: string;
  via: Schema<"DependencyVia">;
  holder: string | undefined;
}

/**
 * Gating, D1: what the node waits on: its own unsatisfied requirements, condition gates and
 * stage opening, then those its ancestors hold for it. What it contains does not hold it (D8).
 */
export function waitingOn(view: Ready, key: string): Holding[] {
  const derived = view.derived.nodes[key];
  const found: Holding[] = [];
  const add = (holder: string, own: boolean) => {
    for (const blocker of view.derived.nodes[holder]?.blocked_by ?? []) {
      if (blocker.via !== "containment" && !found.some((held) => held.node === blocker.node && JSON.stringify(held.via) === JSON.stringify(blocker.via))) {
        found.push({ node: blocker.node, via: blocker.via, holder: own ? undefined : holder });
      }
    }
  };
  add(key, true);
  for (const ancestor of derived?.blocked_through ?? []) {
    add(ancestor, false);
  }
  return found;
}

/** The node's owner by name, when it has one. */
export function ownerName(view: Ready, key: string): string | undefined {
  const [owner] = view.derived.nodes[key]?.participations?.["k_owner"]?.entities ?? [];
  return owner === undefined ? undefined : entityName(view, owner);
}

const SHOWN_MAX = 3;

/** "A", "A and B", "A, B and C", "A, B, C and 2 more". */
function listed(items: Piece[][]): Piece[] {
  const shown = items.slice(0, SHOWN_MAX);
  const more = items.length - shown.length;
  const out: Piece[] = [];
  shown.forEach((item, at) => {
    if (at > 0) {
      out.push(at === shown.length - 1 && more === 0 ? " and " : ", ");
    }
    out.push(...item);
  });
  if (more > 0) {
    out.push(` and ${String(more)} more`);
  }
  return out;
}

/** A node as a phrase, with its link and owner. */
function namedWithOwner(view: Ready, node: string): Piece[] {
  const owner = ownerName(view, node);
  return [{ node }, ...(owner === undefined ? [] : [` (${owner})`])];
}

/** One thing the node waits on, as a phrase with its link and owner. */
export function holdingPhrase(view: Ready, held: Holding): Piece[] {
  if (typeof held.via === "object" && "stage_opening" in held.via) {
    return ["the ", { node: held.via.stage_opening.group }, " stage opening"];
  }
  return namedWithOwner(view, held.node);
}

/** The things the node waits on, as a phrase: "A (Ben) and the B stage opening". */
export function waitingPhrase(view: Ready, key: string): Piece[] {
  const held = waitingOn(view, key);
  // A stage's own opening is what holds it: name the milestone, not the stage itself.
  const own = (each: Holding) => each.holder === undefined && typeof each.via === "object" && "stage_opening" in each.via && each.via.stage_opening.group === key;
  return held.length === 0 ? ["earlier work"] : listed(held.map((each) => (own(each) ? namedWithOwner(view, each.node) : holdingPhrase(view, each))));
}

const cap = (word: string) => word.slice(0, 1).toUpperCase() + word.slice(1);

/** The decisions to name when a node waits on answers: those its relevance reads that are still open. */
function undecidedNames(view: Ready, key: string): Piece[] {
  const open = unansweredOf(view, key);
  const named = open.length > 0 ? open : (view.derived.nodes[key]?.relevance.decisions ?? []);
  return listed(named.map((decision) => [{ node: decision }, " (unanswered)"]));
}

/** What a condition on `key` says it needs, from its own condition or the one of the node it comes from. */
export function appliesIf(view: Ready, detail: NodeDetail): string | undefined {
  const source = nodeOf(view, detail.derived.relevance.condition_on ?? detail.node.key);
  const clause = source?.relevant_when;
  if (clause == null) {
    return undefined;
  }
  const say = (decision: string, value: boolean | string) => {
    const choices = nodeOf(view, decision)?.choices ?? [];
    const choice = choices.find((each) => (typeof each === "string" ? each : each.id) === value);
    return typeof value === "boolean" ? (value ? "yes" : "no") : choice === undefined || typeof choice === "string" ? value : choice.title;
  };
  return conditionPhrase(clause, say, (decision) => titleOf(view, decision));
}

/** D4: why a finished node is stale, in words. */
function staleWords(view: Ready, failures: Schema<"GuardFailure">[]): Piece[] {
  const name = namer(view);
  return failures.flatMap((failure): Piece[] =>
    typeof failure === "object" ? [" Stale: ", { node: failure.open_dependency }, " reopened."] : [` Stale: ${guardFailureText(failure, name)}.`],
  );
}

/** Priority: why the node ranks where it does, in the top one or two terms' words. */
export function rankClause(view: Ready, derived: NodeDerived, facts: RankFacts): string {
  const { position, row } = facts;
  if (position === undefined) {
    return "";
  }
  const parts = row?.rank == null ? [] : rankParts(row.rank, view.inputs.rank).slice(0, 2);
  const slack = derived.dates.slack_days;
  const due = derived.dates.due?.date;
  const phrases = parts.flatMap((part): string[] => {
    switch (part.signal) {
      case "urgency":
        return due === undefined ? (slack == null ? [] : [`must start ${relativeDays(slack)}`]) : [`is due ${relativeDays(daysFrom(due, view.derived.today))}`];
      case "late":
        return slack == null ? [] : [`is ${plural(-slack, "day")} past its latest start`];
      case "gravity":
        return derived.gravity_from.total > 0 ? [`holds up ${plural(derived.gravity_from.total, "node")}`] : [];
      case "leverage":
        return derived.leverage_from.total > 0 ? [`unblocks ${plural(derived.leverage_from.total, "node")}`] : [];
    }
  });
  // Whatever the blend's top terms were, say at least one thing in words: its date, or what it holds up.
  const fallback =
    due !== undefined
      ? [`is due ${relativeDays(daysFrom(due, view.derived.today))}`]
      : derived.gravity_from.total > 0
        ? [`holds up ${plural(derived.gravity_from.total, "node")}`]
        : [];
  const said = phrases.length === 0 ? fallback : phrases;
  return said.length === 0 ? ` Ranks #${String(position)}.` : ` Ranks #${String(position)}: it ${said.join(" and ")}.`;
}

/** A container's roll-up: how much of it is done (the plan's one rule, plan/tree.ts), the decisions still to make, and the open child with the least slack. */
function rollUp(view: Ready, detail: NodeDetail): Piece[] {
  const { done, total, toDecide } = rollups(view).get(detail.node.key) ?? { done: 0, total: 0, toDecide: 0 };
  if (total === 0) {
    return [];
  }
  const counted = detail.children.filter((child) => child.displayState !== "not_relevant" && child.displayState !== "skipped");
  const tightest = counted
    .filter((child) => !isTerminal(child.state))
    .flatMap((child) => {
      const slack = view.derived.nodes[child.key]?.dates.slack_days;
      return slack == null ? [] : [{ key: child.key, slack }];
    })
    .sort((a, b) => a.slack - b.slack)[0];
  const slack: Piece[] =
    tightest === undefined ? [] : ["; ", tightest.slack < 0 ? `${plural(-tightest.slack, "day")} past the latest start of ` : "the tightest item, ", { node: tightest.key }, tightest.slack < 0 ? "" : `, has ${plural(tightest.slack, "day")} to spare`];
  return [`${String(done)} of ${String(total)} done${toDecide === 0 ? "" : `; ${String(toDecide)} to decide`}`, ...slack, ". "];
}

/** The sentence for the node: pieces of text, links, and the Unsnooze action. A container's starts with its roll-up. */
export function sentenceOf(view: Ready, detail: NodeDetail, rank: RankFacts): Piece[] {
  const shown = detail.derived.display_state;
  return shown === "not_relevant" || shown === "skipped" ? stateSentence(view, detail, rank) : [...rollUp(view, detail), ...stateSentence(view, detail, rank)];
}

/** What the node's display state means for it, in pieces. */
function stateSentence(view: Ready, detail: NodeDetail, rank: RankFacts): Piece[] {
  const { derived, record, node } = detail;
  const today = view.derived.today;
  const state = derived.display_state;
  const on = (date: string) => dateWords(date, today);
  const stored = view.journey.graph.state?.snoozes?.[node.key];
  const finished = record.finished_on == null ? "" : ` on ${on(record.finished_on)}`;
  switch (state) {
    case "ready":
      return [`${node.kind === "decision" ? "Ready to decide." : "Ready."}${rankClause(view, derived, rank)}`];
    case "active": {
      const since = record.started_on == null ? "In progress." : `In progress since ${on(record.started_on)}.`;
      return derived.blocked_by?.some((blocker) => blocker.via !== "containment") === true || (derived.blocked_through ?? []).length > 0
        ? [`${since} Started early: still waits on `, ...waitingPhrase(view, node.key), "."]
        : [since];
    }
    case "blocked":
      return ["Waiting on ", ...waitingPhrase(view, node.key), "."];
    case "conditional": {
      const pending = derived.relevance.pending_on ?? [];
      const applies = appliesIf(view, detail);
      const tail = applies === undefined ? "" : ` Applies if ${applies}.`;
      if (pending.length > 0) {
        const behind = [...new Set(pending.flatMap((decision) => view.derived.nodes[decision]?.relevance.decisions ?? []))];
        return [
          "May apply later: depends on ",
          ...listed(pending.map((decision) => [{ node: decision }])),
          ...(behind.length === 0 ? [] : [", which can't be answered until ", ...listed(behind.map((decision) => [{ node: decision }])), behind.length === 1 ? " is" : " are"]),
          `.${tail} Not counted in priority until then.`,
        ];
      }
      return ["May not apply: depends on ", ...undecidedNames(view, node.key), `.${tail}`];
    }
    case "scheduled": {
      const date = derived.dates.effective_date?.date ?? derived.dates.due?.date;
      return [date === undefined ? "Reaches itself when its date comes (auto-reach)." : `Reaches itself on ${on(date)} (auto-reach).`];
    }
    case "snoozed": {
      const via = derived.snoozed_via ?? undefined;
      const target = derived.snoozed;
      const until = target == null ? [] : "date" in target ? [` until ${on(target.date)}`] : [" until ", { node: target.node }, " is done"];
      return via !== undefined && stored === undefined ? ["Snoozed", ...until, ", through ", { node: via }, ". ", { unsnooze: via }] : ["Snoozed", ...until, "."];
    }
    case "done": {
      const verb = node.kind === "decision" ? `Decided${detail.answer === undefined ? "" : ` ${answerText(view, detail.answer, node)}`}` : node.kind === "milestone" ? "Reached" : "Done";
      const unanswered = derived.relevance.value === "undecided" ? ([" May not apply: depends on ", ...undecidedNames(view, node.key), "."] satisfies Piece[]) : [];
      return [`${verb}${finished}.`, ...staleWords(view, derived.stale ?? []), ...unanswered];
    }
    case "skipped": {
      const through = detail.ancestors.find((ancestor) => view.journey.graph.state?.nodes?.[ancestor.key]?.state === "skipped");
      if (record.state !== "skipped" && through !== undefined) {
        return ["Skipped with ", { node: through.key }, "."];
      }
      return [record.skip_reason == null || record.skip_reason === "" ? "Skipped." : `Skipped: ${record.skip_reason.replace(/\.+$/, "")}.`];
    }
    case "not_relevant": {
      const reads = derived.relevance.decisions ?? [];
      const decided = reads.find((decision) => view.journey.graph.state?.answers?.[decision] !== undefined);
      const answer = decided === undefined ? undefined : view.journey.graph.state?.answers?.[decided];
      const because: Piece[] =
        decided === undefined || answer === undefined ? ["Doesn't apply here."] : ["Doesn't apply because ", { node: decided }, ` = ${answerText(view, answer, nodeOf(view, decided))}.`];
      const kept = record.state !== INITIAL_STATE[node.kind] ? [` Recorded ${cap(record.state)}${finished} before it stopped applying; the record is kept.`] : [];
      return [...because, ...kept];
    }
  }
}

/** The sentence as plain text, nodes by title: what tests and the accessible label read. */
export function plainSentence(view: Ready, pieces: Piece[]): string {
  return pieces.map((piece) => (typeof piece === "string" ? piece : "node" in piece ? titleOf(view, piece.node) : `[Unsnooze ${titleOf(view, piece.unsnooze)}]`)).join("");
}
