// C10: why an item ranks where it does. Rank is the documented blend (Priority): each term
// times its constant, so the parts sum to the rank and the largest part is the reason the
// item sits where it does (reasons.ts says it in words).
import type { Schema } from "@cairn/client";

export type NodeRow = Schema<"NodeRow">;
export type RankTerms = Schema<"RankTerms">;
export type RankConstants = Schema<"RankConstants">;

/** One term of the blend: its signal, its normalized term, its constant, and what it adds. */
export interface RankPart {
  signal: "urgency" | "late" | "gravity" | "unlocks";
  term: number;
  weight: number;
  adds: number;
}

/** Priority: the blend's parts that add to `terms.rank`, largest first; ties by the blend's order. */
export function rankParts(terms: RankTerms, constants: RankConstants): RankPart[] {
  const part = (signal: RankPart["signal"], term: number, weight: number): RankPart => ({ signal, term, weight, adds: term * weight });
  const parts = [
    part("urgency", terms.urgency, constants.urgency),
    part("late", terms.late, constants.late),
    part("gravity", terms.gravity_norm, constants.gravity),
    part("unlocks", terms.unlocks_norm, constants.unlocks),
  ];
  return parts.filter((each) => each.adds > 0).sort((left, right) => right.adds - left.adds);
}
