// The shell's notices: what a write newly caused (D7, the consequences notice), shown at the
// moment of the edit and never stored, and what went wrong with one.
import type { Schema } from "@cairn/client";

import { Emitter } from "./emitter.ts";

export type Consequences = Schema<"Consequences">;
export type PatchAnswer = Schema<"PatchAnswer">;

/** One line of a notice: what kind of consequence, and the nodes it names. */
export interface ConsequenceLine {
  kind: "stale" | "shortfall" | "overdue" | "stalled";
  journey: string;
  nodes: string[];
}

export interface Notice {
  id: number;
  tone: "saved" | "problem";
  title: string;
  lines: ConsequenceLine[];
}

/** D7: what an accepted patch newly caused, journey by journey, each kind that has any. */
export function consequenceLines(answer: PatchAnswer): ConsequenceLine[] {
  return answer.outcome === "applied" ? linesOf(answer.consequences ?? {}) : [];
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
    if (caused.stalled != null) {
      lines.push({ kind: "stalled", journey, nodes: [] });
    }
  }
  return lines;
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
