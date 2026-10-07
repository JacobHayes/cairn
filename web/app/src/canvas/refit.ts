// What a canvas fits itself to again (C3): the view it shows, and while its structure is
// edited (5.6) also how many cards it has, so a node added or removed comes into view, while
// a revision that only changes what the cards say keeps the viewport (5.2).

/** The key the canvas refits on. */
export function refitKey(laidOut: { view: string; model: { cards: readonly unknown[] } }, authoring: boolean): string {
  return authoring ? `${laidOut.view}:${String(laidOut.model.cards.length)}` : laidOut.view;
}
