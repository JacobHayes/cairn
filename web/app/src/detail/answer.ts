// B2: what an answer form would send, from what is stored and what has been typed. Nothing is
// preselected: with nothing stored and nothing chosen there is nothing to send, so Save stays
// disabled until the draft differs from what is stored (a different answer, a new entity, or a
// different reason). A different answer starts a new answer: its reason starts empty.
import type { Schema } from "@cairn/client";

import type { AnswerValue } from "./model.ts";

type ChoiceEffect = Schema<"ChoiceEffect">;

/** What the form holds. */
export interface AnswerState {
  /** The recorded answer, resolved through merged entities. */
  stored: AnswerValue | undefined;
  /** The reason it was recorded with. */
  storedReason: string | undefined;
  /** The answer chosen so far; none until something is picked. */
  draft: AnswerValue | undefined;
  /** The reason as typed; none while its field is untouched. */
  reason: string | undefined;
  /** The new entity's name, for an entity answer, as typed. */
  named: string;
}

/** What Save would send. */
export interface Outgoing {
  value: AnswerValue;
  reason: string;
}

/** Whether two answers are the same answer. */
export function sameAnswer(left: AnswerValue | undefined, right: AnswerValue | undefined): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

/** Whether an answer says something: an entity answer names an entity (or a new one), a date has a date. */
function answers(value: AnswerValue, named: string): boolean {
  if ("entity" in value) {
    return value.entity !== "" || named.trim() !== "";
  }
  if ("date" in value) {
    return value.date !== "";
  }
  return true;
}

/** What Save would send now, or undefined while there is nothing new to save. */
export function outgoing(state: AnswerState): Outgoing | undefined {
  const value = state.draft ?? state.stored;
  if (value === undefined || !answers(value, state.named)) {
    return undefined;
  }
  const newAnswer = !sameAnswer(value, state.stored) || state.named.trim() !== "";
  // A reason belongs to its answer alone: a new answer sends only what was typed for it.
  const reason = state.reason ?? (newAnswer ? "" : (state.storedReason ?? ""));
  const reasonChanged = state.reason !== undefined && state.reason.trim() !== (state.storedReason ?? "").trim();
  return newAnswer || reasonChanged ? { value, reason } : undefined;
}

/** The effect of answering `value`, among a decision's per-choice effects: the one whose whole answer it is. */
export function effectOf(effects: readonly ChoiceEffect[] | undefined, value: AnswerValue | undefined): ChoiceEffect | undefined {
  if (effects === undefined || value === undefined) {
    return undefined;
  }
  const normal = (answer: AnswerValue): unknown => ("multi_choice" in answer ? { multi_choice: [...answer.multi_choice].sort() } : answer);
  return effects.find((effect) => JSON.stringify(normal(effect.answer)) === JSON.stringify(normal(value)));
}

/** The effect of picking option `id` of a boolean, single-choice or multi-choice decision: the one named for it. */
export function effectOfOption(effects: readonly ChoiceEffect[] | undefined, id: string): ChoiceEffect | undefined {
  return effects?.find((effect) => effect.choice === id || ("boolean" in effect.answer && id === (effect.answer.boolean ? "yes" : "no")));
}
