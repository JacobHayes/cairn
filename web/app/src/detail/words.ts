// The plain words the inspector says things in (C8, D8): dates as "Due in 3 days", each
// choice's effect beside it, a condition as a phrase, what a save would cause. Pure, so the unit
// tests check the wording without a page. Nothing here is a code or an abbreviation.
import type { Schema } from "@cairn/client";

import { dateWords, dayOf } from "../timeline/model.ts";
import type { NodeKind } from "./model.ts";

type ChoiceEffect = Schema<"ChoiceEffect">;
type Consequences = Schema<"Consequences">;
type Condition = Schema<"ConditionResolved">;

/** "1 node", "3 nodes". */
export function plural(count: number, one: string, many = `${one}s`): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

/** Whole days from `today` to `date`: negative when it has passed. */
export function daysFrom(date: string, today: string): number {
  return dayOf(date) - dayOf(today);
}

/** A moment in days, in words: "today", "tomorrow", "in 5 days", "3 days ago". */
export function relativeDays(days: number): string {
  if (days === 0) {
    return "today";
  }
  if (days === 1) {
    return "tomorrow";
  }
  return days > 0 ? `in ${plural(days, "day")}` : days === -1 ? "yesterday" : `${plural(-days, "day")} ago`;
}

/** How far ahead a date is said in days; beyond it, as a calendar date. */
const SOON_DAYS = 14;

/** The node's one date in words: "Due in 3 days", "Decide by Oct 14", "3 days late". */
export function dueLine(kind: NodeKind, due: string | undefined, today: string): string | undefined {
  if (due === undefined) {
    return undefined;
  }
  const days = daysFrom(due, today);
  const verb = kind === "decision" ? "Decide" : "Due";
  if (days < 0) {
    return `${plural(-days, "day")} late`;
  }
  if (days <= SOON_DAYS) {
    return days === 0 ? `${verb} today` : days === 1 ? `${verb} tomorrow` : `${verb} ${relativeDays(days)}`;
  }
  return kind === "decision" ? `Decide by ${dateWords(due, today)}` : `Due ${dateWords(due, today)}`;
}

/** A node count with its word: "6 nodes". */
function nodes(total: number): string {
  return plural(total, "node");
}

/** One choice's effect, beside it: "brings in 6 · drops 3 (1 has progress)", or "no change". */
export function effectWords(effect: ChoiceEffect): string {
  const found: string[] = [];
  if (effect.brings_in.total > 0) {
    found.push(`brings in ${String(effect.brings_in.total)}`);
  }
  if (effect.drops.total > 0) {
    const progress = effect.drops_with_progress.total;
    found.push(`drops ${String(effect.drops.total)}${progress > 0 ? ` (${String(progress)} ${progress === 1 ? "has" : "have"} progress)` : ""}`);
  }
  if (effect.opens_decisions.total > 0) {
    found.push(`opens ${String(effect.opens_decisions.total)} more ${effect.opens_decisions.total === 1 ? "decision" : "decisions"}`);
  }
  if (effect.decided_later.total > 0) {
    found.push(`${String(effect.decided_later.total)} decided later`);
  }
  return found.length === 0 ? "no change" : found.join(" · ");
}

/**
 * What saving would do, before it is saved: the choice's scope effect, then what the draft patch
 * would newly cause. `name` titles a node. The warning half (D7) is its own sentence.
 */
export function savingWords(effect: ChoiceEffect | undefined, caused: Consequences | undefined, name: (key: string) => string): { info: string | undefined; warning: string | undefined } {
  const info: string[] = [];
  if (effect !== undefined && effect.brings_in.total > 0) {
    info.push(`brings in ${nodes(effect.brings_in.total)}`);
  }
  if (effect !== undefined && effect.drops.total > 0) {
    const progress = effect.drops_with_progress.total;
    info.push(`drops ${nodes(effect.drops.total)}${progress > 0 ? ` (${String(progress)} with recorded progress; ${progress === 1 ? "it is" : "they are"} kept)` : ""}`);
  }
  if (caused?.unlocked !== undefined && caused.unlocked.length > 0) {
    info.push(`frees ${nodes(caused.unlocked.length)} to start`);
  }
  const warning: string[] = [];
  const titles = (keys: string[]) => keys.map(name).join(", ");
  if (caused?.overdue !== undefined && caused.overdue.length > 0) {
    warning.push(`makes ${titles(caused.overdue)} overdue`);
  }
  if (caused?.shortfalls !== undefined && caused.shortfalls.length > 0) {
    warning.push(`leaves ${titles(caused.shortfalls.map((shortfall) => shortfall.node))} short of days`);
  }
  if (caused?.stale !== undefined && caused.stale.length > 0) {
    warning.push(`makes ${titles(caused.stale.map((stale) => stale.node))} stale`);
  }
  if (caused?.undecided !== undefined && caused.undecided.length > 0) {
    warning.push(`means ${titles(caused.undecided.map((each) => each.node))} may not apply`);
  }
  if (caused?.stalled != null) {
    warning.push("leaves nothing that can move");
  }
  return {
    info: info.length === 0 ? undefined : `Saving ${info.join(" and ")}.`,
    warning: warning.length === 0 ? undefined : `${info.length === 0 ? "Saving " : "It "}${warning.join(" and ")}.`,
  };
}

/** The decisions a condition reads, in the order it names them. */
export function conditionDecisions(clause: Condition): string[] {
  if ("equals" in clause) {
    return [clause.equals.decision];
  }
  if ("not_equals" in clause) {
    return [clause.not_equals.decision];
  }
  if ("contains" in clause) {
    return [clause.contains.decision];
  }
  if ("in" in clause) {
    return [clause.in.decision];
  }
  if ("answered" in clause) {
    return [clause.answered];
  }
  if ("not" in clause) {
    return conditionDecisions(clause.not);
  }
  return ("all" in clause ? clause.all : clause.any).flatMap(conditionDecisions);
}

/** What a condition value is called: `value` is a choice id, a boolean, or a plain text. */
type Say = (decision: string, value: boolean | string) => string;

/** A condition as a phrase after "applies if": "Partner", "not Our team", "Partner or Both". */
export function conditionPhrase(clause: Condition, say: Say, title: (decision: string) => string): string {
  if ("equals" in clause) {
    return say(clause.equals.decision, clause.equals.value);
  }
  if ("not_equals" in clause) {
    return `not ${say(clause.not_equals.decision, clause.not_equals.value)}`;
  }
  if ("contains" in clause) {
    return `${title(clause.contains.decision)} includes ${say(clause.contains.decision, clause.contains.value)}`;
  }
  if ("in" in clause) {
    return clause.in.values.map((value) => say(clause.in.decision, value)).join(" or ");
  }
  if ("answered" in clause) {
    return `${title(clause.answered)} is answered`;
  }
  if ("not" in clause) {
    return `not (${conditionPhrase(clause.not, say, title)})`;
  }
  const [joiner, parts] = "all" in clause ? [" and ", clause.all] : [" or ", clause.any];
  return parts.map((part) => conditionPhrase(part, say, title)).join(joiner);
}
