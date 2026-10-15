// D8: the words and badge tones of the engine's display state, in one place so every
// surface says the same thing (mirrored in the MCP instructions, `instructions/SKILL.md`).
// Stored state names (OPEN, TODO, PENDING) are not display words: they stay in the API and in
// History ("recorded Open"). A group says NOT STARTED where a node waits for work to begin;
// a container that is not a group (a deliverable with children) uses its kind's word.
import type { Schema } from "@cairn/client";

import type { NodeKind } from "../detail/model.ts";
import type { Tone } from "../ui/kit.tsx";

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
  milestone: { done: "reached" },
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
