// A card's size, worked out from what it shows before anything is drawn, so the layout (C15)
// reads the same sizes the cards render at and lays out the same graph identically: a head, a
// title of up to two lines, and at most a body line, a foot line and the hidden-prerequisites
// line (5.5). NodeCard draws each row at exactly these heights. What an overlay or the Signals
// lens adds hangs outside the card, so neither moves a node.
import type { LayoutRequest } from "./layout.ts";
import type { Card, CanvasModel } from "./model.ts";

/** A card's width (px): ten of the ruling's cells. */
export const CARD_WIDTH_PX = 240;
/** The padding inside the border, sides and top and bottom (px). */
export const CARD_PAD_X_PX = 12;
export const CARD_PAD_Y_PX = 10;
/** Each row's height, and the gap between rows (px). */
export const HEAD_PX = 24;
export const TITLE_LINE_PX = 21;
export const BODY_PX = 21;
export const FOOT_PX = 22;
export const ROW_GAP_PX = 4;
/** The border's width on each side (px): the box is the border plus the padding plus the rows. */
export const BORDER_PX = 1;

/** Characters that surely fit one line of a title at the card's width. */
export const TITLE_LINE_CHARS = 24;
/** The most lines a title takes; the rest is cut short with an ellipsis. */
export const TITLE_LINES_MAX = 2;

/** The lines a text of `length` characters takes at `perLine` characters a line, at most `max`. */
export function linesFor(length: number, perLine: number, max: number): number {
  return Math.min(Math.max(1, Math.ceil(length / perLine)), max);
}

/** Each row of a card, and whether it is drawn, in the order NodeCard draws them. */
export interface CardRows {
  /** Kind and state. */
  head: true;
  title: number;
  /** A decided decision's answer, a container's progress, or what a conditional node depends on. */
  body: boolean;
  /** The one date, and the owner when it is the viewer's or missing. */
  foot: boolean;
  /** The hidden-prerequisites marker (C2). */
  marker: boolean;
}

/** The rows `card` draws. */
export function cardRows(card: Card): CardRows {
  const journey = card.journey;
  return {
    head: true,
    title: linesFor(card.title.length, TITLE_LINE_CHARS, TITLE_LINES_MAX),
    body: journey?.body !== undefined,
    foot: (journey !== undefined && (journey.foot !== undefined || journey.owner !== undefined)) || card.route?.foot !== undefined,
    marker: card.hiddenPrerequisites.length > 0,
  };
}

/** A card's height for its rows: the rows, the gaps between them, and the padding. */
export function cardHeight(rows: CardRows): number {
  const heights = [HEAD_PX, rows.title * TITLE_LINE_PX, ...(rows.body ? [BODY_PX] : []), ...(rows.foot ? [FOOT_PX] : []), ...(rows.marker ? [BODY_PX] : [])];
  return heights.reduce((sum, each) => sum + each, 0) + (heights.length - 1) * ROW_GAP_PX + 2 * (CARD_PAD_Y_PX + BORDER_PX);
}

/**
 * C15: what the layout places for a canvas: each card at its size in tree order, a container
 * sized by ELK around its children below its own card, and the lines.
 */
export function layoutRequestOf(model: CanvasModel): Omit<LayoutRequest, "hints"> {
  const containers = new Set(model.cards.flatMap((card) => (card.parent === undefined ? [] : [card.parent])));
  return {
    nodes: model.cards.map((card) => {
      const height = cardHeight(cardRows(card));
      return {
        key: card.key,
        ...(card.parent === undefined ? {} : { parent: card.parent }),
        width: CARD_WIDTH_PX,
        height,
        ...(containers.has(card.key) ? { header: height } : {}),
      };
    }),
    edges: model.lines.map((line) => ({ from: line.from, to: line.to })),
  };
}
