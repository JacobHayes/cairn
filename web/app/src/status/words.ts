// D8: the words and badge tones of the engine's display state, in one place so every
// surface says the same thing (mirrored in the MCP instructions, `instructions/SKILL.md`).
// Stored state names (OPEN, TODO, PENDING) are not display words: they stay in the API and in
// History ("recorded Open"). A group says NOT STARTED where a node waits for work to begin;
// a container that is not a group (a deliverable with children) uses its kind's word.
import type { Schema } from "@cairn/client";

import type { NodeKind } from "../detail/model.ts";
import type { SummaryModel } from "../summary/model.ts";
import type { Tone } from "../ui/kit.tsx";
import { plural } from "../ui/words.ts";

export type DisplayState = Schema<"DisplayState">;

/** Every display state, in the order a count lists them: what is moving first, what is out last. */
export const DISPLAY_STATES: readonly DisplayState[] = [
  "active",
  "ready",
  "blocked",
  "conditional",
  "scheduled",
  "snoozed",
  "done",
  "skipped",
  "not_relevant",
];

const WORDS: Record<DisplayState, string> = {
  ready: "ready",
  active: "active",
  blocked: "blocked",
  conditional: "conditional",
  scheduled: "scheduled",
  snoozed: "snoozed",
  done: "done",
  skipped: "skipped",
  not_relevant: "not relevant",
};

/** The word a state takes where the kind says it differently: a decision to decide, a group not started. */
const KIND_WORDS: Partial<Record<NodeKind, Partial<Record<DisplayState, string>>>> = {
  decision: { ready: "to decide", done: "decided" },
  group: { ready: "not started" },
};

/** The state's word for a node of `kind` (lower case; the chip sets it in capitals). */
export function statusWord(state: DisplayState, kind: NodeKind): string {
  return KIND_WORDS[kind]?.[state] ?? WORDS[state];
}

/** The state's word where there is no kind to say it by (a count). */
export function stateWord(state: DisplayState): string {
  return WORDS[state];
}

/** The badge tone each state takes: its dot's colour and shape, so no two states look alike (DESIGN, Status). */
export function statusTone(state: DisplayState): Tone {
  switch (state) {
    case "ready":
      return "ink";
    case "active":
      return "ring";
    case "blocked":
      return "blocked";
    case "conditional":
      return "conditional";
    case "scheduled":
      return "scheduled";
    case "snoozed":
      return "warn";
    case "done":
      return "done";
    case "skipped":
      return "skipped";
    case "not_relevant":
      return "superseded";
  }
}

/**
 * What needs a look in a journey, worst first, said once for every surface that counts it (the
 * journey index and Home, the journey card, the Summary): a count of nodes with what is wrong
 * with them (`2 nodes overdue`), so a count is never taken for days, and its ink (D3, D4, F6).
 */
export const TROUBLES: readonly { flag: "overdue" | "shortfall" | "stale"; words: (count: number) => string; tone: "bad" | "warn"; count: (model: SummaryModel) => number }[] = [
  { flag: "overdue", words: (count) => `${nodesWords(count)} overdue`, tone: "bad", count: (model) => model.overdue.length },
  { flag: "shortfall", words: (count) => `${nodesWords(count)} short of days`, tone: "bad", count: (model) => model.shortfalls.length },
  { flag: "stale", words: (count) => `${nodesWords(count)} stale`, tone: "warn", count: (model) => model.stale.length },
];

function nodesWords(count: number): string {
  return plural(count, "node");
}

/** A word or phrase as a sentence's first words: `the finish of X` becomes `The finish of X`. */
export function upperFirst(text: string): string {
  return `${text.slice(0, 1).toUpperCase()}${text.slice(1)}`;
}

/** What a node's field is called, for every surface that says it changed or what it was set to. */
const FIELD_NAMES: Record<string, string> = {
  id: "id",
  parent: "place",
  title: "title",
  description: "description",
  weight: "weight",
  relevant_when: "condition",
  due_by: "due date rule",
  not_before: "earliest start rule",
  estimate: "estimate",
  placeholder: "placeholder setting",
  requires_artifact: "artifact requirement",
  requires_note: "note requirement",
  final: "final setting",
  auto_done: "marks-itself-done setting",
  opens_at: "opening date",
  closes_at: "closing date",
  gates: "gates",
  closes: "closing stage",
  prompt: "question",
  help: "help text",
  choices: "choices",
  fills_role: "role it fills",
  feeds_milestone: "milestone it feeds",
};

export function fieldWords(field: string): string {
  return FIELD_NAMES[field] ?? field.replaceAll("_", " ");
}
