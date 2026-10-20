// One voice for an answer: a choice is called by its label (never its id), a boolean by yes or
// no, a date in words, an entity by name. Every surface that says what was answered, or what a
// condition compares to, goes through here.
import type { Schema } from "@cairn/client";

import { dateWords } from "./when.ts";

export type Choice = Schema<"Choice">;
type AnswerValue = Schema<"AnswerValue">;

/** A choice's id. */
export function choiceId(choice: Choice): string {
  return typeof choice === "string" ? choice : choice.id;
}

/** A choice as a person reads it: its label, or its id when it has none. */
export function choiceLabel(choice: Choice): string {
  return typeof choice === "string" ? choice : choice.title;
}

/** What the choice `id` of `choices` is called: its label, else (a choice that is gone) the id as written. */
export function choiceNamed(choices: readonly Choice[] | undefined, id: string): string {
  const found = (choices ?? []).find((choice) => choiceId(choice) === id);
  return found === undefined ? id : choiceLabel(found);
}

/** What an answer's choices and entities are called. */
export interface AnswerNames {
  choice: (id: string) => string;
  entity: (key: string) => string;
  /** A date in words; the year shows unless it is today's. */
  date?: (date: string) => string;
}

const listOf = (items: readonly string[]): string => (items.length === 0 ? "none" : items.join(", "));

/** An answer's value in words. */
export function answerWords(value: AnswerValue, names: AnswerNames): string {
  if ("boolean" in value) {
    return value.boolean ? "yes" : "no";
  }
  if ("single_choice" in value) {
    return names.choice(value.single_choice);
  }
  if ("multi_choice" in value) {
    return listOf(value.multi_choice.map((id) => names.choice(id)));
  }
  if ("entity" in value) {
    return names.entity(value.entity);
  }
  if ("entity_list" in value) {
    return listOf(value.entity_list.map((key) => names.entity(key)));
  }
  if ("date" in value) {
    return (names.date ?? dateWords)(value.date);
  }
  return value.text;
}
