// The Signals lens: one labelled chip in each card's bottom-right slot, tinted on the
// neutral ramp by quartile of the range shown. It never re-lays out: the chip hangs outside the
// card. Pure, so the unit tests read the same chips the canvas draws.
import type { Card, Lens } from "./model.ts";

export const LENS_WORDS: Record<Lens, string> = { rank: "Rank" };

/** The one-line meaning of each signal, for the key under the View menu. */
export const LENS_MEANING: Record<Lens, string> = {
  rank: "Where it stands among what can be done now.",
};

/** What a card's chip says, its hover (none when the chip already says it), and the number the tint follows. */
export interface LensChip {
  text: string;
  title: string | undefined;
  value: number;
}

/** The chip the lens puts on `card`, or none when the card has no rank. */
export function lensChip(card: Card): LensChip | undefined {
  const rank = card.journey?.rank;
  return rank === undefined ? undefined : { text: `Rank #${String(rank)}`, title: undefined, value: -rank };
}

/** Each chip's tint: 0 (palest) to 3 (darkest), by quartile of the values shown. A smaller rank is the stronger signal. */
export function lensTiers(chips: ReadonlyMap<string, LensChip>): Map<string, number> {
  const values = [...chips.values()].map((chip) => chip.value).sort((a, b) => a - b);
  const tiers = new Map<string, number>();
  for (const [key, chip] of chips) {
    const below = values.filter((value) => value < chip.value).length;
    tiers.set(key, values.length <= 1 ? 3 : Math.min(3, Math.floor((below / values.length) * 4)));
  }
  return tiers;
}

/** Every card's chip, by key. */
export function lensChips(cards: readonly Card[]): Map<string, LensChip> {
  const chips = new Map<string, LensChip>();
  for (const card of cards) {
    const chip = lensChip(card);
    if (chip !== undefined) {
      chips.set(card.key, chip);
    }
  }
  return chips;
}
