// The plain words the inspector says things in (C8, D8): each
// choice's effect beside it, a condition as a phrase, what a save would cause. Pure, so the unit
// tests check the behaviour without a page. Nothing here is a code or an abbreviation.
import type { Schema } from "@cairn/client";

import type { FormCase } from "./actions-form.ts";
import type { NodeKind } from "./model.ts";

type ChoiceEffect = Schema<"ChoiceEffect">;
type Consequences = Schema<"Consequences">;

/** What a guard waits on, in words: the thing a bypass passes the node without. */
export function guardWords(guard: Schema<"Guard">): string {
  switch (guard) {
    case "deps_done":
      return "what it waits on";
    case "has_artifact":
      return "a link";
    case "has_note":
      return "a note";
    case "broken_down":
      return "a breakdown";
  }
}

/** What a form's write did, as the first words of a block that keeps what was typed after it: "Skipped.", "Marked done.". */
export function keptWords(kind: FormCase, node: NodeKind): string {
  switch (kind) {
    case "skip":
      return "Skipped.";
    case "keep":
      return "Kept.";
    case "include":
      return "Included.";
    case "bypass":
    case "evidence":
      return node === "milestone" ? "Marked reached." : "Marked done.";
    case "anyway":
      return "Answered.";
    case "snooze":
      return "Snoozed.";
    case "edit-date":
      return "Date saved.";
  }
}

/** "1 node", "3 nodes". */
export function plural(count: number, one: string, many = `${one}s`): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

/** A node count with its word: "6 nodes". */
function nodes(total: number): string {
  return plural(total, "node");
}

/** What a choice's effect does to the scope, as phrases: "brings in 6 nodes", "drops 3 nodes (1 with recorded progress; it is kept)". */
function scopeParts(effect: ChoiceEffect): string[] {
  const parts: string[] = [];
  if (effect.brings_in.total > 0) {
    parts.push(`brings in ${nodes(effect.brings_in.total)}`);
  }
  if (effect.drops.total > 0) {
    const progress = effect.drops_with_progress.total;
    parts.push(`drops ${nodes(effect.drops.total)}${progress > 0 ? ` (${String(progress)} with recorded progress; ${progress === 1 ? "it is" : "they are"} kept)` : ""}`);
  }
  return parts;
}

/** One choice's effect, beside it: "brings in 6 nodes · drops 3 nodes", or "changes nothing else". */
export function effectWords(effect: ChoiceEffect): string {
  const found = scopeParts(effect);
  if (effect.opens_decisions.total > 0) {
    found.push(`opens ${String(effect.opens_decisions.total)} more ${effect.opens_decisions.total === 1 ? "decision" : "decisions"}`);
  }
  if (effect.decided_later.total > 0) {
    found.push(`${String(effect.decided_later.total)} decided later`);
  }
  return found.length === 0 ? "changes nothing else" : found.join(" · ");
}

/**
 * What saving would do, before it is saved: the choice's scope effect, then what the draft patch
 * would newly cause. `name` titles a node. The warning half (D7) is its own sentence.
 */
export function savingWords(effect: ChoiceEffect | undefined, caused: Consequences | undefined, name: (key: string) => string): { info: string | undefined; warning: string | undefined } {
  const info = effect === undefined ? [] : scopeParts(effect);
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
