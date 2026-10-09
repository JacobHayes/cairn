// D8: the words, dots and far-zoom glyphs of the engine's display state, in one place so every
// surface says the same thing (mirrored in the MCP instructions, `instructions/SKILL.md`).
// Stored state names (OPEN, TODO, PENDING) are not display words: they stay in the API and in
// History ("recorded Open"). A group says NOT STARTED where a node waits for work to begin;
// a container that is not a group (a deliverable with children) uses its kind's word.
import type { Schema } from "@cairn/client";

import type { NodeKind } from "../detail/model.ts";

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

/** A chip's dot: its colour and its shape, so no two states look alike (DESIGN, Status). */
export interface StatusDot {
  tone: "ink" | "steel" | "warning" | "success" | "none";
  shape: "filled" | "ring" | "square" | "dashed" | "struck" | "hollow";
}

const DOTS: Record<DisplayState, StatusDot> = {
  ready: { tone: "ink", shape: "filled" },
  active: { tone: "ink", shape: "ring" },
  blocked: { tone: "steel", shape: "square" },
  conditional: { tone: "none", shape: "dashed" },
  scheduled: { tone: "steel", shape: "ring" },
  snoozed: { tone: "warning", shape: "filled" },
  done: { tone: "success", shape: "filled" },
  skipped: { tone: "none", shape: "struck" },
  not_relevant: { tone: "none", shape: "hollow" },
};

export function statusDot(state: DisplayState): StatusDot {
  return DOTS[state];
}

/** The glyph a card shows at far zoom, where the word does not fit: its shape alone says the state. */
const GLYPHS: Record<DisplayState, string> = {
  ready: "●",
  active: "◐",
  blocked: "■",
  conditional: "◌",
  scheduled: "◷",
  snoozed: "z",
  done: "✓",
  skipped: "–",
  not_relevant: "○",
};

export function statusGlyph(state: DisplayState): string {
  return GLYPHS[state];
}

/** The badge tone a state reads in: finished is good, held back is a warning, the rest are plain. */
export function statusTone(state: DisplayState): "plain" | "good" | "warn" {
  switch (state) {
    case "done":
      return "good";
    case "blocked":
    case "snoozed":
      return "warn";
    case "ready":
    case "active":
    case "conditional":
    case "scheduled":
    case "skipped":
    case "not_relevant":
      return "plain";
  }
}
