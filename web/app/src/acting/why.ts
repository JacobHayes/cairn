// C10: why an item ranks where it does. Rank is the documented blend (Priority): each term
// times its constant, which the engine serves as what each adds, so the parts sum to the rank
// and the largest part is the reason the item sits where it does (reasons.ts says it in words).
import type { Schema } from "@cairn/client";

export type NodeRow = Schema<"NodeRow">;
export type RankTerms = Schema<"RankTerms">;

/** One term of the blend: its signal, its normalized term, and what it adds. */
export interface RankPart {
  signal: "urgency" | "late" | "gravity" | "unlocks";
  term: number;
  adds: number;
}

/** Priority: the blend's parts that add to `terms.rank`, largest first; ties by the blend's order. */
export function rankParts(terms: RankTerms): RankPart[] {
  const parts: RankPart[] = [
    { signal: "urgency", term: terms.urgency, adds: terms.adds.urgency },
    { signal: "late", term: terms.late, adds: terms.adds.late },
    { signal: "gravity", term: terms.gravity_norm, adds: terms.adds.gravity },
    { signal: "unlocks", term: terms.unlocks_norm, adds: terms.adds.unlocks },
  ];
  return parts.filter((each) => each.adds > 0).sort((left, right) => right.adds - left.adds);
}
