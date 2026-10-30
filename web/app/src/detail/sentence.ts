// C8, D8: the one plain sentence the inspector says under its header, built in the tab from the
// node's display state and what its derive already holds: what its state means for this node
// and, where it helps, what to do about it. Pure, so the unit tests check the wording without a
// page; the pieces carry links, which the component draws.
import { INITIAL_STATE, type Schema } from "@cairn/client";

import { conditionNames, conditionWords } from "../authoring/condition.ts";
import { rankParts, type NodeRow } from "../acting/why.ts";
import { SNOOZE_NODE_TAIL } from "../status/snooze.ts";
import { upperFirst } from "../status/words.ts";
import { dateWords, daysUntil, daysWords, relativeDays } from "../status/when.ts";
import { namer, staleText } from "./explain.ts";
import { itemTitle, nodeOf, rankPlaces, scheduledDate, titleOf, waitsOn, type Holding, type NodeDetail, type NodeDerived, type Ready } from "./model.ts";
import { answerText, identityName, ownersOf, ownerWords } from "./sections.tsx";
import { listParts, plural } from "../ui/words.ts";

/** A part of the sentence: text, a link to a node, or the Unsnooze action for the container whose snooze holds over the node. */
export type Piece = string | { node: string };

/** What the inspector knows of the node's rank: its place on the acting frontier and its terms, once read. */
export interface RankFacts {
  /** One-based place on the ranked acting frontier, none when it is not on it. */
  position: number | undefined;
  row: NodeRow | undefined;
}

/** The place of `key` on the ranked acting frontier, one-based. */
export function positionOf(view: Ready, key: string): number | undefined {
  return rankPlaces(view).get(key);
}

/** The node's owners in words, when it has any. */
export function ownerName(view: Ready, key: string, viewer: readonly string[] = []): string | undefined {
  const owners = ownersOf(view, key);
  return owners.length === 0 ? undefined : ownerWords(view, owners, viewer);
}

/** "A", "A and B", "A, B and C", "A, B and 2 more", as parts a sentence renders. */
function listed(items: Piece[][]): Piece[] {
  return listParts(items).flat();
}

/** A node as a phrase, with its link and owner. */
function namedWithOwner(view: Ready, node: string, viewer: readonly string[]): Piece[] {
  const owner = ownerName(view, node, viewer);
  return [{ node }, ...(owner === undefined ? [] : [` (${owner})`])];
}

/** When a scheduled thing it waits on comes, after its name: " on Nov 1"; nothing for one no date holds. */
function onDate(view: Ready, held: Holding): Piece[] {
  return held.date === undefined ? [] : [` on ${dateWords(held.date, view.derived.today)}`];
}

/** One thing the node waits on, as a phrase with its link and owner, and its date when a date holds it. */
export function holdingPhrase(view: Ready, held: Holding, viewer: readonly string[] = []): Piece[] {
  return [...namedWithOwner(view, held.node, viewer), ...onDate(view, held)];
}

/** The things the node waits on, as a phrase: "A (Ben) and B", "Launch on Nov 1". */
export function waitingPhrase(view: Ready, key: string, viewer: readonly string[] = []): Piece[] {
  const held = waitsOn(view, key);
  return held.length === 0 ? ["earlier work"] : listed(held.map((each) => holdingPhrase(view, each, viewer)));
}

/** D8: what a scheduled node waits for, in a sentence: a milestone marks itself done on its day or is marked done once it comes; work or a decision waits for its own date. */
function scheduledSentence(view: Ready, detail: NodeDetail): Piece[] {
  const { node } = detail;
  const date = scheduledDate(view, node.key);
  const on = date === undefined ? "its date" : dateWords(date, view.derived.today);
  if (node.kind === "milestone") {
    return [node.auto_done === true ? `Scheduled for ${on}; it marks itself done that day.` : `Scheduled for ${on}; mark it done when it happens.`];
  }
  if (node.kind === "decision") {
    return [`Scheduled: to be decided from ${on}.`];
  }
  return [`Scheduled: not before ${on}.`];
}

/** The decisions to name when a node waits on answers: those its relevance reads that are still open. */
function undecidedNames(view: Ready, key: string): Piece[] {
  const open = view.derived.nodes[key]?.relevance.unanswered ?? [];
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
  return conditionWords(clause, conditionNames((decision) => nodeOf(view, decision), (key) => identityName(view, key)));
}

/** D4: why a finished node is stale, in words. */
function staleWords(view: Ready, key: string, reasons: Schema<"StaleReason">[]): Piece[] {
  const name = namer(view);
  return reasons.flatMap((reason): Piece[] =>
    typeof reason === "object" && "reopened" in reason
      ? [" Stale: ", { node: reason.reopened }, " was reopened."]
      : [` Stale: ${staleText(reason, name, (id) => itemTitle(view, key, id))}.`],
  );
}

/** That a node's date has come or gone, after "it": `is due in 5 days`, `is 2 days late`. */
function dueClause(due: string, today: string): string {
  const days = daysUntil(due, today);
  return days < 0 ? `is ${daysWords(-days)} late` : `is due ${relativeDays(days)}`;
}

/** Priority: why the node ranks where it does, in the top one or two terms' words. */
export function rankClause(view: Ready, derived: NodeDerived, facts: RankFacts): string {
  const { position, row } = facts;
  if (position === undefined) {
    return "";
  }
  const parts = row?.rank == null ? [] : rankParts(row.rank).slice(0, 2);
  const slack = derived.dates.slack_days;
  const due = derived.dates.due?.date;
  const phrases = parts.flatMap((part): string[] => {
    switch (part.signal) {
      case "urgency":
        return due === undefined ? (slack == null ? [] : [`must start ${relativeDays(slack)}`]) : [dueClause(due, view.derived.today)];
      case "late":
        return slack == null ? [] : [`is ${plural(-slack, "day")} past its latest start`];
      case "gravity":
        return derived.gravity_from.total > 0 ? [`holds up ${plural(derived.gravity_from.total, "node")}`] : [];
      case "unlocks":
        return derived.unlocks_from.total > 0 ? [`unlocks ${plural(derived.unlocks_from.total, "node")}`] : [];
    }
  });
  // Whatever the blend's top terms were, say at least one thing in words: its date, or what it holds up.
  const fallback =
    due !== undefined
      ? [dueClause(due, view.derived.today)]
      : derived.gravity_from.total > 0
        ? [`holds up ${plural(derived.gravity_from.total, "node")}`]
        : [];
  const said = phrases.length === 0 ? fallback : phrases;
  return said.length === 0 ? ` Ranks #${String(position)}.` : ` Ranks #${String(position)}; it ${said.join(" and ")}.`;
}

/** A container's roll-up: how much of it is done (the plan's one rule, plan/tree.ts), the decisions still to make, and the open child with the least slack. */
function rollUp(view: Ready, detail: NodeDetail): Piece[] {
  const progress = detail.derived.progress;
  if (progress == null || progress.total === 0) {
    return [];
  }
  const { done, total, decisions_needed: toDecide } = progress;
  const tightest = detail.children
    .filter((child) => view.derived.nodes[child.key]?.open === true)
    .flatMap((child) => {
      const slack = view.derived.nodes[child.key]?.dates.slack_days;
      return slack == null ? [] : [{ key: child.key, slack }];
    })
    .sort((a, b) => a.slack - b.slack)[0];
  const slack: Piece[] =
    tightest === undefined ? [] : ["; ", tightest.slack < 0 ? `${plural(-tightest.slack, "day")} past the latest start of ` : "the tightest item, ", { node: tightest.key }, tightest.slack < 0 ? "" : `, has ${plural(tightest.slack, "day")} to spare`];
  return [`${String(done)} of ${String(total)} done${toDecide === 0 ? "" : `; ${String(toDecide)} to decide`}`, ...slack, ". "];
}

/** Whether the node is a group marked done (it reads done and skips what is beneath it): its progress no longer counts what was skipped, so its sentence has no roll-up. */
function markedDone(detail: NodeDetail): boolean {
  return detail.derived.skips_beneath === true && detail.derived.display_state === "done";
}

/** D1a: the kept work beneath a skipped container or a group marked done that is not yet done, which its dependents still wait for. */
function keptPending(derived: NodeDerived): Piece[] {
  const kept = derived.kept_work_pending ?? [];
  return kept.length === 0 ? [] : [" Kept work still pending: ", ...listed(kept.map((node) => [{ node }])), "."];
}

/** The sentence for the node: pieces of text, links, and the Unsnooze action. A container's starts with its roll-up. */
export function sentenceOf(view: Ready, detail: NodeDetail, rank: RankFacts, viewer: readonly string[] = []): Piece[] {
  const shown = detail.derived.display_state;
  return shown === "not_relevant" || shown === "skipped" || markedDone(detail) ? stateSentence(view, detail, rank, viewer) : [...rollUp(view, detail), ...stateSentence(view, detail, rank, viewer)];
}

/** What the node's display state means for it, in pieces. */
function stateSentence(view: Ready, detail: NodeDetail, rank: RankFacts, viewer: readonly string[]): Piece[] {
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
      return [`${record.started_on == null ? "In progress." : `In progress since ${on(record.started_on)}.`}${rankClause(view, derived, rank)}`];
    }
    case "blocked":
      return ["Waiting on ", ...waitingPhrase(view, node.key, viewer), "."];
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
    case "scheduled":
      return scheduledSentence(view, detail);
    case "snoozed": {
      const via = derived.snoozed_via ?? undefined;
      const target = derived.snoozed;
      const until = target == null ? [] : "date" in target ? [` until ${on(target.date)}`] : [" until ", { node: target.node }, SNOOZE_NODE_TAIL];
      return via !== undefined && stored === undefined ? ["Snoozed", ...until, ", through ", { node: via }, "."] : ["Snoozed", ...until, "."];
    }
    case "done": {
      if (markedDone(detail)) {
        const why = record.reason == null || record.reason === "" ? "" : `: ${record.reason.replace(/\.+$/, "")}`;
        const skipped = derived.skip_count ?? 0;
        return [`Marked done${why}.`, ...(skipped === 0 ? [] : [` It skipped ${plural(skipped, "unfinished item")}.`]), ...keptPending(derived)];
      }
      const verb = node.kind === "decision" ? `Decided${detail.answer === undefined ? "" : ` ${answerText(view, detail.answer, node)}`}` : "Done";
      const unanswered = derived.relevance.value === "undecided" ? ([" May not apply: depends on ", ...undecidedNames(view, node.key), "."] satisfies Piece[]) : [];
      return [`${verb}${finished}.`, ...staleWords(view, node.key, derived.stale ?? []), ...unanswered];
    }
    case "skipped": {
      const kept = keptPending(derived);
      const by = derived.skipped_by ?? undefined;
      if (record.state !== "skipped" && by !== undefined) {
        return view.journey.graph.state?.nodes?.[by]?.state === "done" ? ["Skipped: ", { node: by }, " was marked done.", ...kept] : ["Skipped with ", { node: by }, ".", ...kept];
      }
      return [record.reason == null || record.reason === "" ? "Skipped." : `Skipped: ${record.reason.replace(/\.+$/, "")}.`, ...kept];
    }
    case "not_relevant": {
      const reads = derived.relevance.decisions ?? [];
      const decided = reads.find((decision) => view.journey.graph.state?.answers?.[decision] !== undefined);
      const answer = decided === undefined ? undefined : view.journey.graph.state?.answers?.[decided];
      const because: Piece[] =
        decided === undefined || answer === undefined ? ["Doesn't apply here."] : ["Doesn't apply because ", { node: decided }, ` = ${answerText(view, answer, nodeOf(view, decided))}.`];
      const kept = record.state !== INITIAL_STATE[node.kind] ? [` Recorded ${upperFirst(record.state)}${finished} before it stopped applying; the record is kept.`] : [];
      return [...because, ...kept];
    }
  }
}

/** The sentence as plain text, nodes by title: what tests and the accessible label read. */
export function plainSentence(view: Ready, pieces: Piece[]): string {
  return pieces.map((piece) => (typeof piece === "string" ? piece : titleOf(view, piece.node))).join("");
}
