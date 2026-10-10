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

/**
 * An open container's header band (px): one row holding its kind, title, progress, state and
 * fold control, with the date and the hidden-prerequisites marker under it when it has them.
 */
export const BAND_PX = 52;
/** The least width of an open container (px): its band's row needs the room for its title between the rest. */
export const CONTAINER_WIDTH_PX = 384;

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

/** The height of an open container's own header band: its row, and the rows under it that it has, inside its border above and the hairline below. */
export function bandHeight(rows: CardRows): number {
  const heights = [BAND_PX, ...(rows.foot ? [FOOT_PX] : []), ...(rows.marker ? [BODY_PX] : [])];
  return heights.reduce((sum, each) => sum + each, 0) + (heights.length - 1) * ROW_GAP_PX + (heights.length > 1 ? CARD_PAD_Y_PX : 0) + 2 * BORDER_PX;
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
      const rows = cardRows(card);
      return {
        key: card.key,
        ...(card.parent === undefined ? {} : { parent: card.parent }),
        ...(containers.has(card.key) ? { width: CONTAINER_WIDTH_PX, height: bandHeight(rows), header: bandHeight(rows) } : { width: CARD_WIDTH_PX, height: cardHeight(rows) }),
      };
    }),
    edges: model.lines.map((line) => ({ from: line.from, to: line.to })),
  };
}

/** The share of its size an average character of a title takes in width, and a line's height as a share of its size. */
const CHAR_EM = 0.58;
const LEADING_EM = 1.15;
/** Characters a line gives up to words that do not break at its end. */
const WRAP_WASTE_CHARS = 4;
/** The sizes (layout px) a zoomed-out title is set between: the reading size, and the most the room gives. */
export const TITLE_FIT_MIN_PX = 15;
export const TITLE_FIT_MAX_PX = 60;

/**
 * The largest size (layout px) a title of `length` characters is set at, zoomed out, so it fits
 * whole in a box of `width` by `height`: the more room per character, the larger. Never below
 * the reading size, so a title too long for its room is cut short there instead.
 */
export function titleFitPx(length: number, width: number, height: number): number {
  for (let size = TITLE_FIT_MAX_PX; size > TITLE_FIT_MIN_PX; size -= 1) {
    const lines = Math.floor(height / (size * LEADING_EM));
    const perLine = Math.floor(width / (size * CHAR_EM)) - WRAP_WASTE_CHARS;
    if (lines * perLine >= length) {
      return size;
    }
  }
  return TITLE_FIT_MIN_PX;
}

/** The padding a zoomed-out card keeps, the room the glyph takes before a title, and the room a card's controls take after it (px). */
export const ZOOMED_PAD_PX = 6;
export const GLYPH_PX = 20;
export const CONTROL_PX = 30;
export const OPEN_PX = 72;

/** The room (px) a title has zoomed out in a box `width` by `height`: less its padding, the state's glyph if it leads, and the controls beside it. */
export function titleRoom(width: number, height: number, beside: { glyph: boolean; controls: number }): { width: number; height: number } {
  return { width: width - 2 * (BORDER_PX + ZOOMED_PAD_PX) - (beside.glyph ? GLYPH_PX : 0) - beside.controls, height: height - 2 * (BORDER_PX + ZOOMED_PAD_PX) };
}
