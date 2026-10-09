// What this tab did and what it caused (D7): the saves it made, newest first, for the sync
// chip's popover (in memory only), the warning sentence a save's receipt shows at its
// control, and the one toast kept for confirmations that are not about syncing.
import type { Schema } from "@cairn/client";

import { Emitter } from "./emitter.ts";

export type Consequences = Schema<"Consequences">;
export type PatchAnswer = Schema<"PatchAnswer">;

/**
 * One line of a save's consequences: what kind, and the nodes it names. `unanchored` is a
 * route's advisory notice (A20) rather than a journey's consequence: it has no journey, and
 * names the paths of the work no chain links to the final milestone.
 */
export interface ConsequenceLine {
  kind: "stale" | "shortfall" | "overdue" | "undecided" | "stalled" | "unanchored";
  journey: string;
  nodes: string[];
  /** For `undecided`: the open decisions the finished nodes' relevance waits on (D4). */
  unanswered?: string[];
}

/** D7: what an accepted patch newly caused, journey by journey, each kind that has any. */
export function consequenceLines(answer: PatchAnswer): ConsequenceLine[] {
  if (answer.outcome !== "applied") {
    return [];
  }
  const lines = linesOf(answer.consequences ?? {});
  const found = answer.notices ?? [];
  return found.length === 0 ? lines : [...lines, { kind: "unanchored", journey: "", nodes: found.map((notice) => notice.path) }];
}

/** D7: what a write newly caused in each journey it changed, each kind that has any. */
export function linesOf(consequences: Record<string, Consequences>): ConsequenceLine[] {
  const lines: ConsequenceLine[] = [];
  for (const [journey, caused] of Object.entries(consequences)) {
    const add = (kind: ConsequenceLine["kind"], nodes: string[]) => {
      if (nodes.length > 0) {
        lines.push({ kind, journey, nodes });
      }
    };
    add("stale", (caused.stale ?? []).map((stale) => stale.node));
    add("shortfall", (caused.shortfalls ?? []).map((shortfall) => shortfall.node));
    add("overdue", caused.overdue ?? []);
    const undecided = caused.undecided ?? [];
    if (undecided.length > 0) {
      const unanswered = [...new Set(undecided.flatMap((each) => each.unanswered))].sort();
      lines.push({ kind: "undecided", journey, nodes: undecided.map((each) => each.node), unanswered });
    }
    if (caused.stalled != null) {
      lines.push({ kind: "stalled", journey, nodes: [] });
    }
  }
  return lines;
}

/**
 * D4: the warning for finished work whose relevance waits on `unanswered` decisions, named by
 * `title`: it was accepted, and may not apply once they are answered.
 */
export function mayNotApply(unanswered: string[], title: (key: string) => string = (key) => key): string {
  const names = unanswered.map(title).join(", ");
  return `May not apply, since ${names} ${unanswered.length === 1 ? "is" : "are"} unanswered`;
}

/** D7: the node a save acted on and the nodes it newly brought onto the acting frontier (maybe none). */
export interface Unlocks {
  journey: string;
  by: string;
  nodes: string[];
  /** When the write was sent, as `Activity.begin` counted it: a save sent before a screen opened is not that screen's. */
  began: number;
}

/** A node's title, given the journey it is in. */
export type TitleOf = (journey: string, node: string) => string;

/** A list of titles as words: "A", "A and B", "A and 2 more". */
function named(titles: string[]): string {
  const [first = "", second = ""] = titles;
  return titles.length <= 2 ? [first, second].filter((title) => title !== "").join(" and ") : `${first} and ${String(titles.length - 1)} more`;
}

/**
 * D7: the one sentence a save's receipt adds when the write has warning consequences ("Kickoff
 * is now overdue."), or undefined when it has none. What a save merely brought in or unlocked
 * is not a warning and is not said here.
 */
export function warningOf(lines: ConsequenceLine[], titleOf: TitleOf): string | undefined {
  const sentences = lines.map((line) => {
    const titles = line.nodes.map((node) => titleOf(line.journey, node));
    const many = line.nodes.length > 1;
    switch (line.kind) {
      case "stalled":
        return "The journey is now stalled.";
      case "undecided":
        return `${mayNotApply(line.unanswered ?? [], (node) => titleOf(line.journey, node))}.`;
      case "overdue":
        return `${named(titles)} ${many ? "are" : "is"} now overdue.`;
      case "stale":
        return `${named(titles)} ${many ? "are" : "is"} now stale.`;
      case "shortfall":
        return `${named(titles)} now ${many ? "have shortfalls" : "has a shortfall"}.`;
      case "unanchored":
        return `No chain to the final milestone, so neither priority nor dates feel it (advisory): ${line.nodes.join(", ")}.`;
    }
  });
  return sentences.length === 0 ? undefined : sentences.join(" ");
}

/** One save this tab made, for the popover's Recent. */
export interface SaveEvent {
  id: number;
  at: Date;
  /** What was done, in words ("Answered Who runs testing?"). */
  text: string;
  /** The receipt's warning sentence, when the write had warning consequences. */
  warning: string | undefined;
  /** What the write acted on and unlocked, for a pass over the acting frontier (never shown as a warning). */
  unlocks: Unlocks | undefined;
}

/** The saves the popover lists. */
export const SAVE_COUNT_MAX = 10;

/** A confirmation that is not about syncing ("Imported a file"): shown briefly, replaced by the next. */
export interface Toast {
  id: number;
  tone: "confirm" | "problem";
  text: string;
}

export class Activity extends Emitter {
  #saves: readonly SaveEvent[] = [];
  #toast: Toast | undefined;
  #next = 0;
  #began = 0;

  /** How many writes have been sent: a later write has a higher count. */
  get began(): number {
    return this.#began;
  }

  /** Counts a write being sent, and returns its count. */
  begin(): number {
    return ++this.#began;
  }

  /** This tab's last saves, newest first. */
  get saves(): readonly SaveEvent[] {
    return this.#saves;
  }

  get toast(): Toast | undefined {
    return this.#toast;
  }

  saved(event: Omit<SaveEvent, "id">): SaveEvent {
    const saved = { ...event, id: ++this.#next };
    this.#saves = [saved, ...this.#saves].slice(0, SAVE_COUNT_MAX);
    this.emit();
    return saved;
  }

  /** Shows `text` as the toast, replacing the one on screen. */
  confirm(text: string, tone: Toast["tone"] = "confirm"): void {
    this.#toast = { id: ++this.#next, tone, text };
    this.emit();
  }

  /** Takes the toast `id` down, unless a newer one replaced it. */
  clearToast(id: number): void {
    if (this.#toast?.id === id) {
      this.#toast = undefined;
      this.emit();
    }
  }
}
