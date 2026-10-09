// The Signals lens (8.2): one labelled chip in each card's bottom-right slot, tinted on the
// neutral ramp by quartile of the range shown. It never re-lays out: the chip hangs outside the
// card. Pure, so the unit tests read the same chips the canvas draws.
import type { Card, Lens } from "./model.ts";

export const LENS_WORDS: Record<Lens, string> = { rank: "Rank", gravity: "Gravity", unblocks: "Unblocks", slack: "Slack" };

/** The one-line meaning of each signal, for the key under the View menu. */
export const LENS_MEANING: Record<Lens, string> = {
  rank: "Where it stands among what can be done now.",
  gravity: "How much rides on it.",
  unblocks: "How many things finishing it frees.",
  slack: "How many days it can slip before the deadline.",
};

/** What a card's chip says, its hover, and the number the tint follows. */
export interface LensChip {
  text: string;
  title: string;
  value: number;
}

function days(count: number): string {
  return `${String(count)} ${count === 1 ? "day" : "days"}`;
}

/** The chip `lens` puts on `card`, or none when the card has no value for it. */
export function lensChip(card: Card, lens: Lens): LensChip | undefined {
  const signals = card.journey?.signals;
  if (signals === undefined) {
    return undefined;
  }
  switch (lens) {
    case "rank":
      return signals.rank === undefined ? undefined : { text: `Rank #${String(signals.rank)}`, title: `Number ${String(signals.rank)} of what can be done now`, value: -signals.rank };
    case "gravity":
      return signals.gravity === undefined ? undefined : { text: `Gravity ${String(signals.gravity)}`, title: "How much rides on it", value: signals.gravity };
    case "unblocks":
      return signals.unblocks === undefined
        ? undefined
        : { text: `Unblocks ${String(signals.unblocks.count)}`, title: `Frees ${String(signals.unblocks.count)} ${signals.unblocks.count === 1 ? "node" : "nodes"}, weighted ${String(signals.unblocks.weighted)}`, value: signals.unblocks.weighted };
    case "slack":
      return signals.slackDays === undefined ? undefined : { text: `Slack ${days(signals.slackDays)}`, title: "How many days it can slip", value: -signals.slackDays };
  }
}

/** Each chip's tint: 0 (palest) to 3 (darkest), by quartile of the values shown. For rank and slack a smaller number is the stronger signal. */
export function lensTiers(chips: ReadonlyMap<string, LensChip>): Map<string, number> {
  const values = [...chips.values()].map((chip) => chip.value).sort((a, b) => a - b);
  const tiers = new Map<string, number>();
  for (const [key, chip] of chips) {
    const below = values.filter((value) => value < chip.value).length;
    tiers.set(key, values.length <= 1 ? 3 : Math.min(3, Math.floor((below / values.length) * 4)));
  }
  return tiers;
}

/** Every card's chip for `lens`, by key. */
export function lensChips(cards: readonly Card[], lens: Lens): Map<string, LensChip> {
  const chips = new Map<string, LensChip>();
  for (const card of cards) {
    const chip = lensChip(card, lens);
    if (chip !== undefined) {
      chips.set(card.key, chip);
    }
  }
  return chips;
}
