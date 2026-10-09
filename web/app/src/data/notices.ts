// The shell's notices: what a write newly caused (D7, the consequences notice), shown at the
// moment of the edit and never stored, among it the warning that finished work may not apply
// while a decision is unanswered (D4), and what went wrong with one.
import type { Schema } from "@cairn/client";

import { Emitter } from "./emitter.ts";

export type Consequences = Schema<"Consequences">;
export type PatchAnswer = Schema<"PatchAnswer">;

/**
 * One line of a notice: what kind of consequence, and the nodes it names. `unanchored` is a
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

export interface Notice {
  id: number;
  tone: "saved" | "problem";
  title: string;
  lines: ConsequenceLine[];
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

/** The notices on screen, newest last; a few at most. */
export const NOTICE_COUNT_MAX = 3;

export class Notices extends Emitter {
  #notices: readonly Notice[] = [];
  #next = 0;

  get current(): readonly Notice[] {
    return this.#notices;
  }

  add(notice: Omit<Notice, "id">): void {
    this.#next += 1;
    this.#notices = [...this.#notices, { ...notice, id: this.#next }].slice(-NOTICE_COUNT_MAX);
    this.emit();
  }

  dismiss(id: number): void {
    this.#notices = this.#notices.filter((notice) => notice.id !== id);
    this.emit();
  }
}
