import type { Card } from "./model.ts";
import { standsFor } from "./overlay.ts";

// What a canvas fits itself to again (C3): the view it shows, and while its structure is
// edited also how many cards it has, so a node added or removed comes into view, while
// a revision that only changes what the cards say keeps the viewport.

/** The key the canvas refits on. */
export function refitKey(laidOut: { view: string; model: { cards: readonly unknown[] } }, authoring: boolean): string {
  return authoring ? `${laidOut.view}:${String(laidOut.model.cards.length)}` : laidOut.view;
}

/** The cards a first fit to these nodes frames, in the order of the nodes (each card stands for its rolled-up nodes too), or none when no such card is drawn, so the whole graph is fitted. */
export function firstFitCards(keys: readonly string[] | undefined, cards: readonly Card[]): string[] | undefined {
  const ids = new Set((keys ?? []).flatMap((key) => cards.filter((card) => standsFor(card).includes(key)).map((card) => card.key)));
  return ids.size === 0 ? undefined : [...ids];
}
