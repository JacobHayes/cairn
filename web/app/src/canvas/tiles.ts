// The canvas draws a container as a tile when zoomed out (DESIGN, Charts and graphs: Graph
// cards). Below the reading zoom a stage or group is one sheet at its own laid-out box, carrying
// its title, state, progress and due date, and it opens in place once what it holds would be
// readable: at the reading zoom, or earlier when everything directly inside it is itself a
// container at least OPEN_CHILD_PX on screen. The zoom alone decides it: a pan never changes
// it, and nothing moves, since a tile occupies its container's own box. This file is the pure
// part: each container's open zoom, the tiles and the hidden cards at a zoom, the line a tile's
// edge runs along, and what a tile of a given size has room to say.
//
// Cost: O(cards) to build the table of open zooms once per layout and O(cards x depth) per
// distinct tiling; the canvas asks again only when the zoom crosses an open zoom.
import { statusWord } from "../status/words.ts";
import { CHIP_EXTRA_PX, CHIP_LETTER_PX } from "./cards.ts";
import { NEAR_ZOOM } from "./gestures.ts";
import type { Point, Routes } from "./layout.ts";
import { wrappedLines } from "./measure.ts";
import { mergeDateParts } from "./dates.ts";
import { joinLines, STRENGTH, type Card, type DateLine, type Line } from "./model.ts";
import { DATED_MARK, type CanvasOverlay, type LineMark, type OverlayMark } from "./overlay.ts";
import { markedDoneWords, progressWords } from "./words.ts";

/** A container whose children are all containers at least this big on screen (px) opens before the reading zoom. */
export const OPEN_CHILD_PX = { width: 120, height: 40 };

/** What the table reads of a card. */
export interface TileCard {
  key: string;
  parent: string | undefined;
}

/** Each container's open zoom, and each card's container: built once per layout. */
export interface TileTable {
  /** The zoom a container opens at; a card with no children is not in it. */
  opens: ReadonlyMap<string, number>;
  parents: ReadonlyMap<string, string | undefined>;
  /** The open zooms below the reading zoom, ascending: the zooms at which some container opens or closes. */
  thresholds: readonly number[];
}

/**
 * The zoom each container opens at: the reading zoom, or less when every card directly inside it
 * is a container whose laid-out box is OPEN_CHILD_PX or more on screen at that zoom (so the
 * smallest zoom at which they all are).
 */
export function tileTableOf(cards: readonly TileCard[], sizeOf: (key: string) => { width: number; height: number } | undefined): TileTable {
  const children = new Map<string, string[]>();
  for (const card of cards) {
    if (card.parent !== undefined) {
      children.set(card.parent, [...(children.get(card.parent) ?? []), card.key]);
    }
  }
  const opens = new Map<string, number>();
  for (const [key, inside] of children) {
    let zoom = 0;
    for (const child of inside) {
      const size = sizeOf(child);
      if (!children.has(child) || size === undefined || size.width <= 0 || size.height <= 0) {
        zoom = NEAR_ZOOM;
        break;
      }
      zoom = Math.max(zoom, OPEN_CHILD_PX.width / size.width, OPEN_CHILD_PX.height / size.height);
    }
    opens.set(key, Math.min(NEAR_ZOOM, zoom));
  }
  const thresholds = [...new Set(opens.values())].sort((one, other) => one - other);
  return { opens, parents: new Map(cards.map((card) => [card.key, card.parent])), thresholds };
}

/**
 * The least zoom at which `key` is drawn as itself, not as or inside a tile: the zoom every
 * container above it (and it, if it holds cards) has opened by. A card is never drawn smaller
 * than the reading zoom.
 */
export function drawnZoomOf(table: TileTable, key: string): number {
  let zoom = table.opens.get(key) ?? NEAR_ZOOM;
  for (let at = table.parents.get(key), seen = 0; at !== undefined && seen <= table.parents.size; at = table.parents.get(at), seen += 1) {
    zoom = Math.max(zoom, table.opens.get(at) ?? 0);
  }
  return zoom;
}

/**
 * The band `zoom` is in: the largest open zoom at or below it, or 0 under all of them. Two zooms
 * in one band draw the same tiles, so a canvas that reads the band re-renders only when a
 * container opens or closes, not on every zoom tick.
 */
export function bandOf(table: TileTable, zoom: number): number {
  let band = 0;
  for (const threshold of table.thresholds) {
    if (threshold > zoom) {
      break;
    }
    band = threshold;
  }
  return band;
}

/** What is a tile at a zoom, and what each hidden card stands behind. */
export interface Tiling {
  /** The containers drawn as tiles: the outermost ones only. */
  tiles: ReadonlySet<string>;
  /** Each card inside a tile, by the tile that stands for it. */
  hidden: ReadonlyMap<string, string>;
}

/** The tiles at a zoom in `band` (see `bandOf`): containers whose open zoom is above it, less those inside another tile. */
export function tilingIn(table: TileTable, band: number): Tiling {
  const closed = (key: string) => (table.opens.get(key) ?? 0) > band;
  const tiles = new Set<string>();
  const hidden = new Map<string, string>();
  for (const key of table.parents.keys()) {
    // The outermost closed container at or above the card.
    let outer: string | undefined;
    for (let at: string | undefined = key, seen = 0; at !== undefined && seen <= table.parents.size; at = table.parents.get(at), seen += 1) {
      if (closed(at)) {
        outer = at;
      }
    }
    if (outer === key) {
      tiles.add(key);
    } else if (outer !== undefined) {
      hidden.set(key, outer);
    }
  }
  return { tiles, hidden };
}

/** The card that stands for `key` on screen: the tile it is inside or is, else itself. */
export function standsBehind(tiling: Tiling, key: string): string {
  return tiling.hidden.get(key) ?? key;
}

// ---- what a tile says ---------------------------------------------------------------------

/** The sizes a tile's title is set at (px on screen), by the room the tile has. */
export const TILE_TITLE_STEPS = [13, 16, 20, 24] as const;
export type TileTitlePx = (typeof TILE_TITLE_STEPS)[number];

/** The padding a tile keeps round its block (px on screen), and the least a tile stacks a second sheet or draws corner ticks at. */
export const TILE_PAD_PX = 8;
export const TILE_STACK_MIN_PX = 40;
export const TILE_TICKS_MIN_PX = 100;

const LEADING = 1.2;
export const TILE_GAP_PX = 4;
/** A body line is a little taller than its text (a meter, a chip). */
export const TILE_BODY_ROW = 1.55;
/** Average width of a body character, in ems. */
const BODY_CHAR_EM = 0.52;
const MONO_CHAR_EM = 0.6;
const METER_MIN_PX = 20;

/** What a tile has to say, as the card knows it. A missing piece is none. */
export interface TilePieces {
  title: string;
  /** The state's word, for the chip. */
  chip: string | undefined;
  /** The progress words ("9 of 12 done"), and whether there is a meter to draw (a count). */
  words: string | undefined;
  meter: boolean;
  /** The container's exceptional message ("overdue"). */
  message: string | undefined;
  /** Its own due date, in the card foot's words. */
  date: string | undefined;
  /** The one fact: what is selected inside, or how many search matches it holds. */
  fact: string | undefined;
}

/** What a tile of a given size draws. */
export interface TileLayout {
  /** The title is drawn; false is a bare sheet (its name stays in its tooltip and accessible name). */
  title: boolean;
  titlePx: TileTitlePx;
  /** The lines the title may take: all of them, or as many as the room holds. */
  titleLines: number;
  bodyPx: number;
  chip: boolean;
  /** The chip sits on a row of its own below the progress, where it has no room beside the title without costing it a line. */
  chipBelow: boolean;
  words: boolean;
  meter: boolean;
  message: boolean;
  date: boolean;
  fact: boolean;
  /** A second sheet 4px behind, from TILE_STACK_MIN_PX. */
  stack: boolean;
  /** Corner ticks, from TILE_TICKS_MIN_PX. */
  ticks: boolean;
}

/** What a card's tile has to say: its title, state, progress, exceptional message and due date as its card says them, and the one fact (`fact`: what is selected inside, or its search matches). */
export function tilePiecesOf(card: Card, fact: string | undefined): TilePieces {
  const journey = card.journey;
  const body = journey?.body;
  const foot = journey?.foot;
  return {
    title: card.title,
    chip: journey === undefined ? undefined : statusWord(journey.state, card.kind),
    words: body?.kind === "progress" ? progressWords(body.done, body.total) : body?.kind === "marked_done" ? markedDoneWords(body.keptPending, body.skipped) : undefined,
    meter: body?.kind === "progress",
    message: body?.kind === "progress" ? body.badge?.flag : undefined,
    date: foot === undefined ? card.route?.foot : `${foot.words}${foot.tail ?? ""}`,
    fact,
  };
}

const BARE: Omit<TileLayout, "stack" | "ticks"> = { title: false, titlePx: 13, titleLines: 1, bodyPx: 12, chip: false, chipBelow: false, words: false, meter: false, message: false, date: false, fact: false };

/** The step a tile of `width` by `height` px sets its title at, at most. */
function stepOf(width: number, height: number): number {
  return width >= 300 && height >= 150 ? 3 : width >= 220 && height >= 100 ? 2 : width >= 160 && height >= 70 ? 1 : 0;
}

/** The lines a title wraps to in `room` px at `px`, or none when its first word is wider than the room. */
const linesOf = (text: string, room: number, px: number) => wrappedLines(text, room / px, true);

const bodyWidth = (text: string, bodyPx: number) => text.length * bodyPx * BODY_CHAR_EM;
/** A date line is set in the mono face, wider than the body face per character. */
const dateWidth = (text: string, bodyPx: number) => text.length * bodyPx * MONO_CHAR_EM;

type Drawn = Pick<TileLayout, "chip" | "chipBelow" | "words" | "meter" | "message" | "date" | "fact">;

/** The rows a tile draws below its title, as heights and the width each needs (px on screen). */
function restOf(pieces: TilePieces, drawn: Drawn, bodyPx: number): { height: number; width: number }[] {
  const body = bodyPx * TILE_BODY_ROW;
  const rows: { height: number; width: number }[] = [];
  if (drawn.meter || drawn.words) {
    rows.push({ height: body, width: drawn.words && pieces.words !== undefined ? bodyWidth(pieces.words, bodyPx) + METER_MIN_PX + TILE_GAP_PX : METER_MIN_PX });
  }
  if (drawn.chip && drawn.chipBelow && pieces.chip !== undefined) {
    rows.push({ height: body, width: chipWidthOf(pieces.chip) });
  }
  for (const [shown, text, widthOf] of [[drawn.message, pieces.message, bodyWidth], [drawn.date, pieces.date, dateWidth], [drawn.fact, pieces.fact, bodyWidth]] as const) {
    if (shown && text !== undefined) {
      rows.push({ height: body, width: widthOf(text, bodyPx) });
    }
  }
  return rows;
}

const chipWidthOf = (word: string) => word.length * CHIP_LETTER_PX + CHIP_EXTRA_PX;

/**
 * What a tile `width` by `height` px on screen has room to draw: the title at the largest step
 * the size allows and its first word fits, then the other pieces in order, each kept while the
 * block still fits the tile. Short of room, pieces go in this order: the fact, the date, the chip,
 * the progress words (and the exceptional message with them), the meter. The title goes last: a
 * tile with no room for its title's first word draws nothing, a bare sheet. The chip sits beside
 * the title unless that costs the title a line; then it takes a row of its own, if that fits.
 */
export function tileLayoutOf(size: { width: number; height: number }, pieces: TilePieces): TileLayout {
  const place = { stack: Math.min(size.width, size.height) >= TILE_STACK_MIN_PX, ticks: Math.min(size.width, size.height) >= TILE_TICKS_MIN_PX };
  const room = { width: size.width - 2 * TILE_PAD_PX, height: size.height - 2 * TILE_PAD_PX };
  for (let step = stepOf(size.width, size.height); step >= 0 && room.width > 0 && room.height > 0; step -= 1) {
    const titlePx = TILE_TITLE_STEPS[step] ?? 13;
    const alone = linesOf(pieces.title, room.width, titlePx);
    // The title needs its first word whole, and a line.
    if (alone === undefined || titlePx * LEADING > room.height) {
      continue;
    }
    const bodyPx = Math.min(15, Math.max(12, Math.round(titlePx * 0.62)));
    const drawn: Drawn = { chip: pieces.chip !== undefined, chipBelow: false, words: pieces.words !== undefined, meter: pieces.meter, message: pieces.message !== undefined, date: pieces.date !== undefined, fact: pieces.fact !== undefined };
    const fits = (): boolean => {
      // The chip never costs the title a line: beside it only where the title keeps its lines, else on its own row.
      if (drawn.chip && pieces.chip !== undefined) {
        const beside = linesOf(pieces.title, room.width - chipWidthOf(pieces.chip) - TILE_GAP_PX, titlePx);
        drawn.chipBelow = beside === undefined || beside > alone;
      }
      const rest = restOf(pieces, drawn, bodyPx);
      const height = alone * titlePx * LEADING + rest.reduce((sum, row) => sum + row.height + TILE_GAP_PX, 0);
      return height <= room.height && rest.every((row) => row.width <= room.width);
    };
    for (const piece of ["fact", "date", "chip", "words", "meter"] as const) {
      if (fits()) {
        break;
      }
      drawn[piece] = false;
      if (piece === "words") {
        drawn.message = false;
      }
    }
    const rest = restOf(pieces, drawn, bodyPx);
    const left = room.height - rest.reduce((sum, row) => sum + row.height + TILE_GAP_PX, 0);
    return { title: true, titlePx, titleLines: Math.min(alone, Math.max(1, Math.floor(left / (titlePx * LEADING)))), bodyPx, ...drawn, ...place };
  }
  return { ...BARE, ...place };
}

// ---- the line a tile's edge runs along ------------------------------------------------------

/** A box on the canvas. */
interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

const inside = (point: Point, box: Rect) => point.x >= box.x && point.x <= box.x + box.width && point.y >= box.y && point.y <= box.y + box.height;

/** Where the segment from `from` (inside `box`) to `to` (outside it) leaves the box. */
function exitOf(from: Point, to: Point, box: Rect): Point {
  let [low, high] = [0, 1];
  for (let step = 0; step < 24; step += 1) {
    const mid = (low + high) / 2;
    const point = { x: from.x + (to.x - from.x) * mid, y: from.y + (to.y - from.y) * mid };
    if (inside(point, box)) {
      low = mid;
    } else {
      high = mid;
    }
  }
  return { x: from.x + (to.x - from.x) * high, y: from.y + (to.y - from.y) * high };
}

/** The part of `route` from where it leaves `box` (its start is inside it). */
function outerStart(route: readonly Point[], box: Rect): Point[] {
  const first = route.findIndex((point) => !inside(point, box));
  const before = route[first - 1];
  const at = route[first];
  // A route that never leaves the box has no outer part; one that starts outside it is all outer.
  if (first < 0) {
    return [];
  }
  if (first === 0 || before === undefined || at === undefined) {
    return [...route];
  }
  return [exitOf(before, at, box), ...route.slice(first)];
}

/**
 * The route a line between a tile and another card (or two tiles) runs: the outer part of one
 * member line's route, the part outside the tile at the end it stands for. `from` and `to` are the
 * boxes of the tiles at either end, none where that end is a card drawn whole. A route that never
 * leaves its tile, or has no point left, is a straight line between the boxes' sides.
 */
export function rolledRoute(member: readonly Point[], ends: { from: Rect | undefined; to: Rect | undefined }, boxes: { from: Rect; to: Rect }): Point[] {
  let route = [...member];
  if (ends.from !== undefined) {
    route = outerStart(route, ends.from);
  }
  if (ends.to !== undefined) {
    route = outerStart([...route].reverse(), ends.to).reverse();
  }
  if (route.length < 2) {
    const [one, other] = [boxes.from, boxes.to];
    const rightward = one.x + one.width <= other.x;
    const start = { x: rightward ? one.x + one.width : one.x, y: one.y + one.height / 2 };
    const end = { x: rightward ? other.x : other.x + other.width, y: other.y + other.height / 2 };
    return [start, { x: (start.x + end.x) / 2, y: start.y }, { x: (start.x + end.x) / 2, y: end.y }, end];
  }
  return route;
}

/** The lines to draw at a tiling, each with its route: lines whose ends are inside a tile run to the tile (`joinLines`), along the outer part of one member's route. */
export interface TiledLines {
  lines: Line[];
  routes: Routes;
  /** The lines each drawn line stands for, by its id. */
  members: ReadonlyMap<string, Line[]>;
}

/** The member whose route a joined line is drawn along: the strongest kind, then the first by id, so the choice never changes between renders. */
function chosenOf(members: readonly Line[], routes: Routes): Line | undefined {
  return members
    .filter((member) => routes[member.id] !== undefined)
    .sort((one, other) => STRENGTH[other.kind] - STRENGTH[one.kind] || (one.id < other.id ? -1 : one.id > other.id ? 1 : 0))[0];
}

/**
 * C1: the lines at a tiling. A line with an end inside a tile is drawn to the tile; lines inside one tile are not drawn;
 * lines between the same two drawn ends are one, in the strongest member's style, which keeps
 * every edge its members stand for. With no tile every line is as it was. `boxes` are the cards' boxes on the canvas.
 */
export function tileLinesOf(lines: readonly Line[], tiling: Tiling, routes: Routes, boxes: ReadonlyMap<string, Rect>): TiledLines {
  if (tiling.tiles.size === 0) {
    return { lines: [...lines], routes, members: new Map() };
  }
  const drawn: Line[] = [];
  const out: Routes = {};
  const members = new Map<string, Line[]>();
  for (const { line, members: standing } of joinLines(lines, (key) => standsBehind(tiling, key))) {
    drawn.push(line);
    members.set(line.id, standing);
    const [from, to] = [boxes.get(line.from), boxes.get(line.to)];
    const member = chosenOf(standing, routes);
    const route = member === undefined ? undefined : routes[member.id];
    if (route === undefined || from === undefined || to === undefined || member === undefined) {
      continue;
    }
    out[line.id] = line.id === member.id ? route : rolledRoute(route, { from: line.from === member.from ? undefined : from, to: line.to === member.to ? undefined : to }, { from, to });
  }
  return { lines: drawn, routes: out, members };
}

/**
 * The overlay at a tiling: a tile that stands for a card the overlay marks is in focus as that
 * card would be, and hangs the tag it would (needs, unblocks), and a line between tiles is lit as its strongest
 * member is (ink over accent over the rest), so a trace, a proposal's layer or Frontier read the
 * one mapping and nothing under a tile is left dimmed.
 */
export function tileOverlay(overlay: CanvasOverlay | undefined, tiling: Tiling, tiled: TiledLines): CanvasOverlay | undefined {
  if (overlay === undefined || tiling.tiles.size === 0) {
    return overlay;
  }
  const marks = { ...overlay.marks };
  const held = new Map<string, OverlayMark[]>();
  for (const [key, tile] of tiling.hidden) {
    const mark = overlay.marks[key];
    if (mark !== undefined && overlay.marks[tile] === undefined) {
      held.set(tile, [...(held.get(tile) ?? []), mark]);
    }
  }
  for (const [tile, inside] of held) {
    // The tile hangs the tag its members would: one word where they agree, their words together where they do not; a tile holding only quiet or diff marks is in focus and says nothing.
    const tagged = inside.filter((mark) => mark.quiet !== true && mark.diff === undefined);
    const [first] = tagged;
    marks[tile] = first === undefined ? DATED_MARK : { tone: first.tone, label: [...new Set(tagged.map((mark) => mark.label))].join(" \u00b7 "), ...(tagged.every((mark) => mark.contributor === true) ? { contributor: true as const } : {}) };
  }
  const lines: Record<string, LineMark> = {};
  const order: LineMark[] = ["ink", "accent", "lit", "add", "remove"];
  for (const [id, standing] of tiled.members) {
    const lit = standing.flatMap((member) => (overlay.lines[member.id] === undefined ? [] : [overlay.lines[member.id] as LineMark])).sort((one, other) => order.indexOf(one) - order.indexOf(other))[0];
    if (lit !== undefined) {
      lines[id] = lit;
    }
  }
  return { ...overlay, marks, lines };
}

/** C1: the selected node's date lines at a tiling: a line to a card inside a tile runs to the tile, lines inside one tile are not drawn, and lines between the same two drawn cards are one that keeps every relationship. */
export function tiledDates(dates: readonly DateLine[], tiling: Tiling): DateLine[] {
  if (tiling.tiles.size === 0) {
    return [...dates];
  }
  return mergeDateParts(
    dates.flatMap((line) => {
      const [from, to] = [standsBehind(tiling, line.from), standsBehind(tiling, line.to)];
      return from === to ? [] : [{ from, to, orders: line.orders, entries: line.entries }];
    }),
  );
}
