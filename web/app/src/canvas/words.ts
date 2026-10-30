// What a card says in plain words (the density call): the one date in its foot, a
// condition's "if" sentence, a snooze's target, a container's progress. Pure and apart from the
// components, so the unit tests read the same sentences the canvas draws.
import { conditionWords, decisionOf, decisionsIn, type ConditionNames } from "../authoring/condition.ts";
import { isLeaf } from "../authoring/graph.ts";
import type { GraphNode, NodeKind } from "../detail/model.ts";
import { dateWords, daysUntil, dueWords, isSoon, lateWords, NEAR_DAYS, SOON_DAYS } from "../status/when.ts";
import type { DisplayState } from "../status/words.ts";
import type { Line, LineKind, LineLink, Tone } from "./model.ts";

/** What a card's foot knows about its node. */
export interface FootFacts {
  kind: NodeKind;
  state: DisplayState;
  today: string;
  due: string | undefined;
  latestStart: string | undefined;
  /** D3, F6: how many days past its due date, as the engine derived it; none when it is not overdue. */
  overdueDays: number | undefined;
  /** Still to do and in scope (`open`): a finished or ruled-out node has no date to give. */
  open: boolean;
  /** A milestone's effective date. */
  doneOn: string | undefined;
}

/**
 * The one date a card's foot gives, in words, with the tone it reads in: how late when late;
 * "Start by" when the node has not started and must within two weeks; else its due words
 * (warning ink within a week). A finished or ruled-out node has none (a milestone says
 * when it was done).
 */
export function footWords(facts: FootFacts): { words: string; tone: Tone } | undefined {
  const { kind, state, today, due, latestStart, overdueDays, open, doneOn } = facts;
  if (kind === "milestone" && state === "done") {
    return doneOn === undefined ? undefined : { words: `Done ${dateWords(doneOn, today)}`, tone: "plain" };
  }
  if (!open) {
    return undefined;
  }
  if (overdueDays !== undefined) {
    return { words: lateWords(overdueDays), tone: "bad" };
  }
  const started = state === "active";
  if (latestStart !== undefined && !started && !isLeaf(kind) && daysUntil(latestStart, today) <= NEAR_DAYS) {
    const left = daysUntil(latestStart, today);
    return { words: `Start by ${dateWords(latestStart, today)}`, tone: left < 0 ? "bad" : left <= SOON_DAYS ? "warn" : "plain" };
  }
  if (due === undefined) {
    return kind === "milestone" && doneOn !== undefined ? { words: dateWords(doneOn, today), tone: "plain" } : undefined;
  }
  return { words: dueWords(due, today, kind === "decision"), tone: isSoon(due, today) ? "warn" : "plain" };
}

/** Progress as words: `3 of 10 done`. */
export function progressWords(done: number, total: number): string {
  return `${String(done)} of ${String(total)} done`;
}

/** What a marked-done container says: its kept work still pending, or how many of its nodes were skipped. */
export function markedDoneWords(keptPending: boolean, skipped: number): string {
  return keptPending ? "Kept work pending" : skipped === 0 ? "Marked done" : `Marked done \u00b7 ${String(skipped)} skipped`;
}

/** A checklist's progress as words: `7 of 11 ticked`. */
export function checklistWords(ticked: number, total: number): string {
  return `${String(ticked)} of ${String(total)} ticked`;
}

/**
 * What a conditional node depends on, as the words after "If": the comparison
 * (`"Who runs testing?" is Partner`) when it is a plain one, else the decisions it reads
 * (`Who runs testing? and Budget`).
 */
export function dependsWords(node: GraphNode, names: ConditionNames): string | undefined {
  const condition = node.relevant_when;
  if (condition == null) {
    return undefined;
  }
  return decisionOf(condition) === undefined ? decisionsIn(condition).map((decision) => names.title(decision)).join(" and ") : conditionWords(condition, names);
}

/** The first paragraph of a markdown rationale as plain text, for a hover. */
export function firstParagraph(markdown: string): string {
  const first = markdown.trim().split(/\n\s*\n/)[0] ?? "";
  return first.replace(/[*_`#>]|\[([^\]]*)\]\([^)]*\)/g, (_, text: string | undefined) => text ?? "").replace(/\s+/g, " ").trim();
}

/** What each kind of line is called, and how the Key says to tell it apart: one table, for the Key and for a line's card. */
export const LINE_KIND_WORDS: Record<LineKind, { label: string; key: string }> = {
  requires: { label: "Needs", key: "the line ends in an arrow." },
  condition: { label: "Applies only if", key: "the line ends in a hollow diamond." },
};

/** What the Key says of a date relationship, which is drawn for the selected node only and is not a kind of edge. */
export const DATE_LINE_KEY = "Dates: a dashed line, shown for the selected node; an open arrowhead marks the later end when the order is fixed.";

/**
 * C1: what one edge says, in words: why the dependent waits on the requirement. The ends are whatever the caller passes (plain titles for a
 * hover, links for a card), so the two say the same thing.
 */
export function edgeSentence<T>(edge: Pick<LineLink, "kind" | "holds">, dependent: T, requirement: T): (T | string)[] {
  switch (edge.kind) {
    case "requires":
      return edge.holds ? [dependent, " needs ", requirement, ", and so does everything inside it"] : [dependent, " needs ", requirement];
    case "condition":
      return [dependent, " applies only depending on ", requirement, ", and waits for it"];
  }
}

/** The edge's sentence as one string, for a hover. */
export function edgeWords(edge: Pick<LineLink, "kind" | "holds">, dependent: string, requirement: string): string {
  return edgeSentence(edge, dependent, requirement).join("");
}

/** Whether a line still holds anything back, in words; none where there is no state. */
export function lineWaitWords(line: Pick<Line, "satisfied">): string | undefined {
  return line.satisfied === undefined ? undefined : line.satisfied ? "holds nothing back now" : "still waiting";
}

/** Whether an edge still holds its dependent back, in words; none where there is no state. */
export function waitWords(edge: Pick<LineLink, "waits">): string | undefined {
  if (edge.waits === undefined) {
    return undefined;
  }
  return edge.waits ? "still waiting" : "holds nothing back now";
}
