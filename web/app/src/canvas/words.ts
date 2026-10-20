// What a card says in plain words (5.5, the density call): the one date in its foot, a
// condition's "if" sentence, a snooze's target, a container's progress. Pure and apart from the
// components, so the unit tests read the same sentences the canvas draws.
import type { GraphNode, NodeKind } from "../detail/model.ts";
import { dateWords, daysUntil, dueWords, isSoon, NEAR_DAYS, SOON_DAYS } from "../status/when.ts";
import type { DisplayState } from "../status/words.ts";
import type { Tone } from "./model.ts";
import { NODE_GONE } from "../ui/gone.ts";

/** What a card's foot knows about its node. */
export interface FootFacts {
  kind: NodeKind;
  state: DisplayState;
  today: string;
  due: string | undefined;
  latestStart: string | undefined;
  overdue: boolean;
  /** A milestone's effective date. */
  reached: string | undefined;
}

/**
 * The one date a card's foot gives, in words, with the tone it reads in: how late when late;
 * "Start by" when the node has not started and must within two weeks; else its due words
 * (warning ink within a week). A finished or ruled-out node has none (a milestone says
 * when it was reached).
 */
export function footWords(facts: FootFacts): { words: string; tone: Tone } | undefined {
  const { kind, state, today, due, latestStart, overdue, reached } = facts;
  if (kind === "milestone" && state === "done") {
    return reached === undefined ? undefined : { words: `Reached ${dateWords(reached, today)}`, tone: "plain" };
  }
  if (state === "done" || state === "skipped" || state === "not_relevant") {
    return undefined;
  }
  if (due !== undefined && overdue) {
    return { words: dueWords(due, today), tone: "bad" };
  }
  const started = state === "active";
  if (latestStart !== undefined && !started && kind !== "decision" && kind !== "milestone" && daysUntil(latestStart, today) <= NEAR_DAYS) {
    const left = daysUntil(latestStart, today);
    return { words: `Start by ${dateWords(latestStart, today)}`, tone: left < 0 ? "bad" : left <= SOON_DAYS ? "warn" : "plain" };
  }
  if (due === undefined) {
    return kind === "milestone" && reached !== undefined ? { words: dateWords(reached, today), tone: "plain" } : undefined;
  }
  return { words: dueWords(due, today, kind === "decision"), tone: isSoon(due, today) ? "warn" : "plain" };
}

/** Progress as words: `3 of 10 done`. */
export function progressWords(done: number, total: number): string {
  return `${String(done)} of ${String(total)} done`;
}

type Condition = NonNullable<GraphNode["relevant_when"]>;

/** The decisions a condition reads, in order, once each. */
function decisionsOf(condition: Condition): string[] {
  const keys: string[] = [];
  const walk = (each: Condition) => {
    if ("equals" in each) {
      keys.push(each.equals.decision);
    } else if ("not_equals" in each) {
      keys.push(each.not_equals.decision);
    } else if ("contains" in each) {
      keys.push(each.contains.decision);
    } else if ("in" in each) {
      keys.push(each.in.decision);
    } else if ("answered" in each) {
      keys.push(each.answered);
    } else if ("all" in each) {
      each.all.forEach(walk);
    } else if ("any" in each) {
      each.any.forEach(walk);
    } else {
      walk(each.not);
    }
  };
  walk(condition);
  return [...new Set(keys)];
}

/** A compared value as people read it: a choice by its label, a boolean as yes or no. */
function valueWords(nodes: readonly GraphNode[], decision: string, value: boolean | string): string {
  if (typeof value === "boolean") {
    return value ? "yes" : "no";
  }
  const choices = nodes.find((node) => node.key === decision)?.choices ?? [];
  const found = choices.find((choice) => (typeof choice === "string" ? choice : choice.id) === value);
  return found === undefined || typeof found === "string" ? value : found.title;
}

/**
 * What a conditional node depends on, as the words after "If": `Who runs testing? = Partner`
 * for a plain comparison, else the decisions it reads (`Who runs testing? and Budget`).
 */
export function dependsWords(nodes: readonly GraphNode[], node: GraphNode): string | undefined {
  const condition = node.relevant_when;
  const title = (key: string) => nodes.find((each) => each.key === key)?.title ?? NODE_GONE;
  if (condition == null) {
    return undefined;
  }
  if ("equals" in condition) {
    return `${title(condition.equals.decision)} = ${valueWords(nodes, condition.equals.decision, condition.equals.value)}`;
  }
  if ("not_equals" in condition) {
    return `${title(condition.not_equals.decision)} is not ${valueWords(nodes, condition.not_equals.decision, condition.not_equals.value)}`;
  }
  if ("in" in condition) {
    return `${title(condition.in.decision)} = ${condition.in.values.map((value) => valueWords(nodes, condition.in.decision, value)).join(" or ")}`;
  }
  return decisionsOf(condition).map(title).join(" and ");
}

/** The first paragraph of a markdown rationale as plain text, for a hover. */
export function firstParagraph(markdown: string): string {
  const first = markdown.trim().split(/\n\s*\n/)[0] ?? "";
  return first.replace(/[*_`#>]|\[([^\]]*)\]\([^)]*\)/g, (_, text: string | undefined) => text ?? "").replace(/\s+/g, " ").trim();
}
