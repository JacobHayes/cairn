// A card's size, worked out from what it shows before anything is drawn, so the layout (C15)
// reads the same sizes the cards render at and lays out the same graph identically: every
// line of a card has a fixed height, a title or prompt takes the lines its length needs (up
// to a cap, the rest cut short), and NodeCard draws each line at exactly these heights. What
// an overlay or the heat toggle adds hangs outside the card, so neither moves a node.
import type { LayoutRequest } from "./layout.ts";
import type { Card, CanvasModel } from "./model.ts";

/** A card's width, and the height of one line of it (px). */
export const CARD_WIDTH_PX = 248;
export const LINE_PX = 18;
/** Padding inside the border, and the heaviest border gravity gives (C6) (px). */
export const CARD_PAD_PX = 8;
export const BORDER_MAX_PX = 4;
/** The room for text inside a card (px). */
export const CARD_TEXT_PX = CARD_WIDTH_PX - 2 * (CARD_PAD_PX + BORDER_MAX_PX) - 4;

/** Characters that surely fit one line of a title, and of a prompt, at the card's width. */
export const TITLE_LINE_CHARS = 26;
export const PROMPT_LINE_CHARS = 30;
/** The most lines a title, and a prompt, take; the rest is cut short with an ellipsis. */
export const TITLE_LINES_MAX = 2;
export const PROMPT_LINES_MAX = 2;
/** C4: the checklist items a card lists before "and N more". */
export const CHECKLIST_SHOWN_MAX = 6;
/** A badge's width per character, and its padding plus the gap after it (px). */
export const BADGE_CHAR_PX = 6.4;
export const BADGE_EXTRA_PX = 18;
/** The most rows of badges a card shows. */
export const BADGE_ROWS_MAX = 3;

/** The lines a text of `length` characters takes at `perLine` characters a line, at most `max`. */
export function linesFor(length: number, perLine: number, max: number): number {
  return Math.min(Math.max(1, Math.ceil(length / perLine)), max);
}

/** The rows a run of badges takes at the card's width, at most `BADGE_ROWS_MAX`. */
export function badgeRows(flags: string[]): number {
  let rows = 0;
  let used = CARD_TEXT_PX;
  for (const flag of flags) {
    const width = flag.length * BADGE_CHAR_PX + BADGE_EXTRA_PX;
    if (used + width > CARD_TEXT_PX) {
      rows += 1;
      used = 0;
    }
    used += width;
  }
  return Math.min(rows, BADGE_ROWS_MAX);
}

/** Each part of a card and the lines it takes, in the order NodeCard draws them. */
export interface CardLines {
  /** Kind and state, with the rank badge. */
  head: number;
  title: number;
  /** Owner (journeys). */
  owner: number;
  /** Due, latest start, and slack (journeys). */
  dates: number;
  prompt: number;
  /** A decision's answer (journeys). */
  answer: number;
  badges: number;
  /** A container's children: the most gravity, the least slack, the owners (C2). */
  children: number;
  checklist: number;
  /** The hidden-prerequisites marker (C2). */
  marker: number;
}

/** The lines each part of `card` takes. */
export function cardLines(card: Card): CardLines {
  const journey = card.journey;
  const checklist = card.checklist.length;
  return {
    head: 1,
    title: linesFor(card.title.length, TITLE_LINE_CHARS, TITLE_LINES_MAX),
    owner: journey === undefined ? 0 : 1,
    dates: journey !== undefined && (journey.due !== undefined || journey.latestStart !== undefined) ? 1 : 0,
    prompt: card.prompt === undefined ? 0 : linesFor(card.prompt.length, PROMPT_LINE_CHARS, PROMPT_LINES_MAX),
    answer: card.kind === "decision" && journey !== undefined ? 1 : 0,
    badges: journey === undefined ? 0 : badgeRows(journey.badges.map((badge) => badge.flag)),
    children: journey?.children === undefined ? 0 : 1,
    checklist: Math.min(checklist, CHECKLIST_SHOWN_MAX) + (checklist > CHECKLIST_SHOWN_MAX ? 1 : 0),
    marker: card.hiddenPrerequisites.length === 0 ? 0 : 1,
  };
}

/** A card's height for its lines: the lines, the padding, and room for the heaviest border. */
export function cardHeight(lines: CardLines): number {
  const count = (Object.values(lines) as number[]).reduce((sum, each) => sum + each, 0);
  return count * LINE_PX + 2 * (CARD_PAD_PX + BORDER_MAX_PX);
}

/**
 * C15: what the layout places for a canvas: each card at its size in tree order, a container
 * sized by ELK around its children below its own card, and the lines.
 */
export function layoutRequestOf(model: CanvasModel): Omit<LayoutRequest, "hints"> {
  const containers = new Set(model.cards.flatMap((card) => (card.parent === undefined ? [] : [card.parent])));
  return {
    nodes: model.cards.map((card) => {
      const height = cardHeight(cardLines(card));
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
