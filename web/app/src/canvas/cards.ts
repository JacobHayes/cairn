// A card's size, worked out from what it shows before anything is drawn, so the layout (C15)
// reads the same sizes the cards render at and lays out the same graph identically: a head, a
// title of up to two lines, and at most a body line, a foot line and the hidden-prerequisites
// line (5.5). NodeCard draws each row at exactly these heights. A title's lines are counted from
// its width as the browser measures it (measure.ts), so the count is what the drawing wraps to. What an overlay or the Signals
// lens adds hangs outside the card, so neither moves a node.
import type { LayoutRequest } from "./layout.ts";
import { emWidthOf } from "./measure.ts";
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

/** The hidden-prerequisites marker's row under a container's rule (px): a line and the padding below it. */
export const MARKER_ROW_PX = BODY_PX + CARD_PAD_Y_PX;

/** The most lines a title takes; the rest is cut short with an ellipsis. */
export const TITLE_LINES_MAX = 2;
/** A card's title is set at this size (px), in the room between the padding. */
export const TITLE_PX = 15;
const TITLE_ROOM_PX = CARD_WIDTH_PX - 2 * (CARD_PAD_X_PX + BORDER_PX);

/**
 * The lines a card's title takes: how many it wraps to between its words in the card's room,
 * at most TITLE_LINES_MAX. This is the one rule: layout sizes the card by it and NodeCard clips
 * the title to exactly this many lines, so what is drawn never outgrows the height laid out. A
 * word wider than a line is cut, so it counts as the most.
 */
export function titleLinesOf(title: string): number {
  return Math.min(wrappedLines(title, TITLE_ROOM_PX / TITLE_PX) ?? TITLE_LINES_MAX, TITLE_LINES_MAX);
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
    title: titleLinesOf(card.title),
    body: journey?.body !== undefined,
    foot: (journey !== undefined && (journey.foot !== undefined || journey.owner !== undefined)) || card.route?.foot !== undefined,
    marker: card.hiddenPrerequisites.length > 0,
  };
}

/** Whether an open container's head has a foot line: its date (or, in an insertion, the segment it came from), but never its owner. */
export function headFoot(card: Card): boolean {
  return card.journey?.foot !== undefined || card.route?.foot !== undefined;
}

/**
 * The height of an open container's own head: kind and state, the title, the progress and the
 * foot line, inside its padding, the border above and the rule below, and under the rule the
 * hidden-prerequisites marker when it has one.
 */
export function headHeight(card: Card): number {
  const rows = cardRows(card);
  const heights = [HEAD_PX, rows.title * TITLE_LINE_PX, ...(rows.body ? [BODY_PX] : []), ...(headFoot(card) ? [FOOT_PX] : [])];
  return heights.reduce((sum, each) => sum + each, 0) + (heights.length - 1) * ROW_GAP_PX + 2 * CARD_PAD_Y_PX + 2 * BORDER_PX + (rows.marker ? MARKER_ROW_PX : 0);
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
      const height = containers.has(card.key) ? headHeight(card) : cardHeight(rows);
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

/** A zoomed-out title's line height as a share of its size. */
const LEADING_EM = 1.15;
/** The sizes (layout px) a zoomed-out container title is set between: the most its room gives, and the least a narrow head can shrink to. */
export const TITLE_FIT_MAX_PX = 60;
export const TITLE_FIT_MIN_PX = 5;
/** A zoomed-out title is held at the reading size (layout px) while its room allows, and at this size on screen (px) as the view zooms out. */
export const TITLE_READING_PX = 15;
export const TITLE_SCREEN_PX = 13;

const wordsOf = (title: string) => title.split(/\s+/).filter(Boolean).map(emWidthOf);

/** The lines a title takes wrapped between its words in `width` ems, or undefined when a word is wider than a line. */
function wrappedLines(title: string, width: number): number | undefined {
  let lines = 1;
  let used = 0;
  const space = emWidthOf(" ");
  for (const word of wordsOf(title)) {
    if (word > width) {
      return undefined;
    }
    if (used === 0) {
      used = word;
    } else if (used + space + word <= width) {
      used += space + word;
    } else {
      lines += 1;
      used = word;
    }
  }
  return lines;
}

/** Whether a word of the title is wider than `width` even at the least size: it can only be cut, with an ellipsis, on one line. */
export function titleWordOverflows(title: string, width: number): boolean {
  return wrappedLines(title, width / TITLE_FIT_MIN_PX) === undefined;
}

/** The whole lines a title of `size` px sets in a room `height` tall. */
export function titleLines(size: number, height: number): number {
  return Math.max(1, Math.floor(height / (size * LEADING_EM)));
}

/**
 * The largest size (layout px) a zoomed-out title is set at so it wraps between its words and fits
 * whole in a box of `width` by `height`: the more room, the larger, down to the least a narrow
 * head shrinks to, where a title too long for its room is cut short at a whole line instead.
 */
export function titleFitPx(title: string, width: number, height: number): number {
  for (let size = TITLE_FIT_MAX_PX; size > TITLE_FIT_MIN_PX; size -= 1) {
    const lines = wrappedLines(title, width / size);
    if (lines !== undefined && lines <= titleLines(size, height) && lines * size * LEADING_EM <= height) {
      return size;
    }
  }
  return TITLE_FIT_MIN_PX;
}

/** The padding a zoomed-out container head keeps around its title, sides and top and bottom (px). */
export const HEAD_PAD_X_PX = 20;
export const HEAD_PAD_Y_PX = 6;

/** The room (px) a title has in a zoomed-out container head of `header` px (its marker row included) in a card `width` wide. */
export function headTitleBox(width: number, header: number, marker: boolean): { width: number; height: number } {
  return { width: width - 2 * (BORDER_PX + HEAD_PAD_X_PX), height: header - 2 * BORDER_PX - (marker ? MARKER_ROW_PX : 0) - 2 * HEAD_PAD_Y_PX };
}

/** A zoomed-out title's size (layout px): the reading size, or 13px on screen when that is larger, but never more than its room allows. */
export function titleSizePx(title: string, box: { width: number; height: number }, zoom: number): number {
  return Math.min(Math.max(TITLE_READING_PX, TITLE_SCREEN_PX / zoom), titleFitPx(title, box.width, box.height));
}

/** The state chip beside a zoomed-out head's title is set at this size on screen (px), with this line height (a multiple of it) and border, at about this width per letter, plus its padding, dot and gap; CSS reads the first three from the chip's style. */
export const CHIP_FONT_PX = 10.5;
export const CHIP_LEADING = 1.6;
export const CHIP_BORDER_PX = 1;
const CHIP_LETTER_PX = 6.9;
const CHIP_EXTRA_PX = 31;

/** Whether a chip of `letters` fits beside the title in `box` at `zoom` without the title getting smaller or taking more lines. */
export function headChipFits(title: string, letters: number, box: { width: number; height: number }, zoom: number): boolean {
  const width = box.width - (letters * CHIP_LETTER_PX + CHIP_EXTRA_PX) / zoom;
  if (width <= 0 || (CHIP_FONT_PX * CHIP_LEADING + 2 * CHIP_BORDER_PX) / zoom > box.height) {
    return false;
  }
  const size = titleSizePx(title, box, zoom);
  return titleSizePx(title, { width, height: box.height }, zoom) >= size && wrappedLines(title, width / size) === wrappedLines(title, box.width / size);
}
