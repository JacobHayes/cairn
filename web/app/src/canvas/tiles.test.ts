// A container zoomed out is a tile (tiles.ts): when it opens, what each hidden card stands behind,
// the lines drawn to a tile, and what a tile of a given size has room to say.
import { describe, expect, it } from "vitest";

import { NEAR_ZOOM } from "./gestures.ts";
import { rolledLine, lineOf, type Card, type DateLine, type Line, type LineLink, type CanvasModel } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";
import { OPEN_CHILD_PX, TILE_STACK_MIN_PX, TILE_TICKS_MIN_PX, bandOf, drawnZoomOf, rolledRoute, standsBehind, tileLayoutOf, tileLinesOf, tileOverlay, tilePiecesOf, tileTableOf, tiledDates, tilingIn, type TilePieces } from "./tiles.ts";

const card = (key: string, parent?: string): Card => ({ key, kind: "action", title: key, parent, stage: false, collapsed: false, rolledUp: [], hiddenPrerequisites: [], matched: false, journey: undefined });
const edge = (requirement: string, dependent: string, extra: Partial<LineLink> = {}): LineLink => ({ requirement, dependent, kind: "requires", holds: false, sentence: "", ...extra });
const line = (from: string, to: string, extra: Partial<LineLink> = {}): Line => lineOf(`${from}->${to}`, from, to, [edge(from, to, extra)]);

/** A stage holding two groups, each holding two cards; a card beside them. */
const cards = [card("stage"), card("group_a", "stage"), card("a1", "group_a"), card("a2", "group_a"), card("group_b", "stage"), card("b1", "group_b"), card("loose")];
const sizes: Record<string, { width: number; height: number }> = { stage: { width: 900, height: 400 }, group_a: { width: 400, height: 200 }, group_b: { width: 400, height: 200 } };
const table = tileTableOf(cards, (key) => sizes[key] ?? { width: 240, height: 80 });

describe("open zooms", () => {
  it("opens a container of loose cards at the reading zoom, and one holding only containers earlier, once each is big enough on screen", () => {
    expect(table.opens.get("group_a")).toBe(NEAR_ZOOM);
    // The groups are 400 x 200: 120 px wide on screen at 0.3.
    expect(table.opens.get("stage")).toBeCloseTo(OPEN_CHILD_PX.width / 400);
  });

  it("opens a container later when its smallest container child is smaller, never past the reading zoom", () => {
    const narrow = tileTableOf(cards, (key) => (key === "group_b" ? { width: 250, height: 200 } : (sizes[key] ?? { width: 240, height: 80 })));
    expect(narrow.opens.get("stage")).toBeCloseTo(OPEN_CHILD_PX.width / 250);
    const tiny = tileTableOf(cards, (key) => (key === "group_b" ? { width: 20, height: 20 } : (sizes[key] ?? { width: 240, height: 80 })));
    expect(tiny.opens.get("stage")).toBe(NEAR_ZOOM);
  });

  it("opens a container that holds a loose card at the reading zoom, whatever else it holds", () => {
    const mixed = tileTableOf([...cards, card("note", "stage")], (key) => sizes[key] ?? { width: 240, height: 80 });
    expect(mixed.opens.get("stage")).toBe(NEAR_ZOOM);
  });

  it("makes nothing a tile at the reading zoom or above", () => {
    for (const zoom of [NEAR_ZOOM, 0.9, 1.5]) {
      expect(tilingIn(table, bandOf(table, zoom)).tiles.size).toBe(0);
    }
  });
});

describe("the zoom a card is drawn as itself at", () => {
  const nested = [card("outer"), card("note", "outer"), card("mid", "outer"), card("inner", "mid"), card("leaf", "inner")];
  const big = tileTableOf(nested, () => ({ width: 1000, height: 600 }));

  it("is the zoom the card's own container and every container above it have opened by, so framing a container never leaves it inside a tile", () => {
    // mid holds one large container and opens early; outer also holds a loose card and waits for the reading zoom.
    expect(big.opens.get("mid")).toBeLessThan(NEAR_ZOOM);
    expect(drawnZoomOf(big, "mid")).toBe(NEAR_ZOOM);
    expect(tilingIn(big, bandOf(big, drawnZoomOf(big, "mid"))).hidden.has("mid")).toBe(false);
  });

  it("is the reading zoom for a card, and the container's own open zoom for one with nothing closed above it", () => {
    expect(drawnZoomOf(big, "leaf")).toBe(NEAR_ZOOM);
    expect(drawnZoomOf(big, "outer")).toBe(NEAR_ZOOM);
    const early = tileTableOf([card("top"), card("mid", "top"), card("inner", "mid"), card("leaf", "inner")], () => ({ width: 1000, height: 600 }));
    expect(drawnZoomOf(early, "mid")).toBe(early.opens.get("mid"));
    expect(drawnZoomOf(early, "mid")).toBeLessThan(NEAR_ZOOM);
  });
});

describe("tiles at a zoom", () => {
  it("draws only the outermost closed container, with everything inside standing behind it", () => {
    const far = tilingIn(table, bandOf(table, 0.1));
    expect([...far.tiles]).toEqual(["stage"]);
    expect(standsBehind(far, "a1")).toBe("stage");
    expect(standsBehind(far, "group_b")).toBe("stage");
    expect(standsBehind(far, "loose")).toBe("loose");
  });

  it("opens the stage first and then its groups as the zoom rises", () => {
    const middle = tilingIn(table, bandOf(table, 0.4));
    expect([...middle.tiles].sort()).toEqual(["group_a", "group_b"]);
    expect(standsBehind(middle, "a1")).toBe("group_a");
    expect(standsBehind(middle, "stage")).toBe("stage");
  });

  it("is the same for every zoom in a band, so the canvas re-renders only when a container opens", () => {
    expect(bandOf(table, 0.35)).toBe(bandOf(table, 0.45));
    expect(bandOf(table, 0.2)).not.toBe(bandOf(table, 0.4));
  });
});

describe("lines drawn to tiles", () => {
  const routes = { "a1->b1": [{ x: 380, y: 100 }, { x: 450, y: 100 }, { x: 450, y: 300 }, { x: 520, y: 300 }] };
  const boxes = new Map([["group_a", { x: 0, y: 0, width: 400, height: 200 }], ["group_b", { x: 500, y: 250, width: 400, height: 200 }], ["a1", { x: 20, y: 40, width: 240, height: 80 }], ["b1", { x: 520, y: 290, width: 240, height: 80 }], ["loose", { x: 0, y: 500, width: 240, height: 80 }]]);
  const tiling = tilingIn(table, bandOf(table, 0.4));

  it("draws lines between the same two tiles as one that keeps every edge, and none inside a tile", () => {
    const lines = [line("a1", "b1"), line("a2", "b1"), line("a1", "a2"), line("loose", "a1")];
    const tiled = tileLinesOf(lines, tiling, { ...routes }, boxes);
    expect(tiled.lines.map((each) => each.id).sort()).toEqual(["group_a->group_b", "loose->group_a"]);
    const joined = tiled.lines.find((each) => each.id === "group_a->group_b");
    expect(joined?.edges.map((each) => `${each.requirement}>${each.dependent}`)).toEqual(["a1>b1", "a2>b1"]);
    expect(tiled.members.get("group_a->group_b")?.map((each) => each.id)).toEqual(["a1->b1", "a2->b1"]);
  });

  it("leaves every line as it was where nothing is a tile", () => {
    const lines = [line("a1", "b1")];
    const open = tileLinesOf(lines, tilingIn(table, bandOf(table, 0.7)), routes, boxes);
    expect(open.lines).toEqual(lines);
    expect(open.routes).toBe(routes);
  });

  it("runs a line to a tile along the outer part of a member's route, ending on the tile's edge", () => {
    const tiled = tileLinesOf([line("a1", "b1")], tiling, { ...routes }, boxes);
    const route = tiled.routes["group_a->group_b"];
    expect(route?.[0]?.x).toBeCloseTo(400, 0);
    expect(route?.at(-1)?.x).toBeCloseTo(500, 0);
    // Between the two tiles, not through either.
    expect(route?.every((point) => !(point.x > 1 && point.x < 399 && point.y > 1 && point.y < 199))).toBe(true);
  });

  it("falls back to a straight run between the tiles when a route never leaves them", () => {
    const route = rolledRoute([{ x: 10, y: 10 }, { x: 20, y: 20 }], { from: boxes.get("group_a"), to: boxes.get("group_b") }, { from: boxes.get("group_a") ?? { x: 0, y: 0, width: 0, height: 0 }, to: boxes.get("group_b") ?? { x: 0, y: 0, width: 0, height: 0 } });
    expect(route[0]).toEqual({ x: 400, y: 100 });
    expect(route.at(-1)).toEqual({ x: 500, y: 350 });
  });

  it("opens a joined line where a line it stands for opens, and not where none does", () => {
    const [one, two] = [line("a1", "b1"), line("a2", "b1")];
    const tileOf = (members: Parameters<typeof tileLinesOf>[0]) => tileLinesOf(members, tiling, {}, new Map()).lines.find((each) => each.id === "group_a->group_b");
    expect(tileOf([{ ...one, selectable: false }, { ...two, selectable: false }])?.selectable).toBe(false);
    expect(tileOf([{ ...one, selectable: false }, two])?.selectable).toBe(true);
  });

  it("is the line the model finds between two cards holding others, listing every edge, the direct link and the links between what they hold alike", () => {
    const direct = line("group_a", "group_b");
    const both = rolledLine({ cards, lines: [direct, line("a1", "b1")] }, { from: "group_a", to: "group_b" });
    expect(both?.edges).toHaveLength(2);
  });

  it("is the line the model finds between two cards holding others, listing every edge", () => {
    const model: CanvasModel = { cards, lines: [line("a1", "b1"), line("a2", "b1")] };
    const found = rolledLine(model, { from: "group_a", to: "group_b" });
    expect(found?.edges).toHaveLength(2);
    expect(rolledLine(model, { from: "group_b", to: "group_a" })).toBeUndefined();
  });
});

describe("the overlay and the date lines at a tiling", () => {
  const tiling = tilingIn(table, bandOf(table, 0.4));
  const tiled = tileLinesOf([line("a1", "b1")], tiling, {}, new Map());

  it("keeps a tile in focus when a card it stands for is marked, and lights its line as its member is", () => {
    const overlay: CanvasOverlay = { title: "Trace", marks: { a1: { tone: "accent", label: "needs" } }, lines: { "a1->b1": "ink" }, dim: true, outside: [] };
    const lit = tileOverlay(overlay, tiling, tiled);
    expect(lit?.marks["group_a"]?.label).toBe("needs");
    expect(lit?.marks["group_b"]).toBeUndefined();
    expect(lit?.lines["group_a->group_b"]).toBe("ink");
  });

  it("hangs on a tile the words its members' tags say together, and nothing for a quiet mark", () => {
    const overlay: CanvasOverlay = { title: "Trace", marks: { a1: { tone: "ink", label: "needs" }, a2: { tone: "accent", label: "unblocks", contributor: true }, b1: { tone: "plain", label: "dates", quiet: true } }, lines: {}, dim: true, outside: [] };
    const lit = tileOverlay(overlay, tiling, tiled);
    expect(lit?.marks["group_a"]?.label).toBe("needs \u00b7 unblocks");
    expect(lit?.marks["group_a"]?.contributor).toBeUndefined();
    expect(lit?.marks["group_b"]?.quiet).toBe(true);
  });

  it("runs a date line to the tile and merges what lands between the same two cards", () => {
    const dateLine = (from: string, to: string, orders: boolean): DateLine => ({ id: `${from}->${to}`, from, to, orders, entries: [{ link: {} as DateLine["entries"][number]["link"], words: `${from} to ${to}` }], sentence: `${from} to ${to}` });
    const merged = tiledDates([dateLine("a1", "b1", false), dateLine("a2", "b1", true), dateLine("a1", "a2", true), dateLine("loose", "a1", false)], tiling);
    expect(merged.map((each) => `${each.from}>${each.to}`).sort()).toEqual(["group_a>group_b", "loose>group_a"]);
    const between = merged.find((each) => each.from === "group_a");
    expect(between?.entries).toHaveLength(2);
    expect(between?.orders).toBe(true);
  });
});

describe("what a tile says", () => {
  const pieces: TilePieces = { title: "Build the parts", chip: "BLOCKED", words: "9 of 12 done", meter: true, message: "overdue", date: "Due Feb 5, 2027", fact: "Component two selected" };
  const drawn = (width: number, height: number, from: TilePieces = pieces) => tileLayoutOf({ width, height }, from);
  const kept = (layout: ReturnType<typeof drawn>) => (["fact", "date", "chip", "words", "meter"] as const).filter((piece) => layout[piece]);

  it("says everything in a roomy tile, and gives way in the order fact, date, chip, progress words, meter as the room shrinks", () => {
    const order = ["fact", "date", "chip", "words", "meter"];
    expect(kept(drawn(420, 240))).toEqual(order);
    let previous = order.length;
    for (const height of [200, 150, 120, 100, 90, 80, 70, 60, 50, 44, 38]) {
      const now = kept(drawn(300, height));
      // What is left is always what comes last in the order, so a piece is dropped only after every piece before it.
      expect(now).toEqual(order.slice(order.length - now.length));
      expect(now.length).toBeLessThanOrEqual(previous);
      previous = now.length;
    }
    expect(previous).toBeLessThan(order.length);
  });

  it("keeps the title after every other piece, and is a bare sheet when its first word has no room", () => {
    const small = drawn(150, 34);
    expect(small.title).toBe(true);
    expect(kept(small)).toEqual([]);
    const bare = drawn(40, 30);
    expect(bare.title).toBe(false);
    expect(kept(bare)).toEqual([]);
    expect(drawn(300, 100, { ...pieces, title: "Unbroken".concat("x".repeat(60)) }).title).toBe(false);
  });

  it("draws a title whose first word fits even when a later word is wider than the tile", () => {
    const layout = drawn(200, 120, { ...pieces, title: "Build ".concat("x".repeat(80)) });
    expect(layout.title).toBe(true);
    expect(layout.titleLines).toBeGreaterThan(1);
  });

  it("never draws the meter or a chip without the title: there is no meter-only tile", () => {
    for (const width of [20, 40, 60, 80, 120, 200]) {
      for (const height of [16, 24, 32, 48, 64]) {
        const layout = drawn(width, height);
        if (!layout.title) {
          expect(kept(layout)).toEqual([]);
        }
      }
    }
  });

  it("sets the title larger in a larger tile, in steps of 13, 16, 20 and 24px", () => {
    const sizes = [[120, 50], [200, 90], [260, 120], [360, 200]].map(([width, height]) => drawn(width ?? 0, height ?? 0).titlePx);
    expect(sizes).toEqual([13, 16, 20, 24]);
  });

  it("stacks a second sheet from 40px and draws corner ticks from 100px", () => {
    expect([drawn(39, 39).stack, drawn(TILE_STACK_MIN_PX, TILE_STACK_MIN_PX).stack]).toEqual([false, true]);
    expect([drawn(99, 200).ticks, drawn(TILE_TICKS_MIN_PX, TILE_TICKS_MIN_PX).ticks]).toEqual([false, true]);
  });

  it("puts the chip beside the title where the title keeps its lines, and on a row of its own where it would cost it one", () => {
    const long = { ...pieces, title: "Settle the long list of small items", fact: undefined, date: undefined, message: undefined };
    const states = [160, 200, 240, 300, 360, 420, 500, 600].map((width) => drawn(width, 200, long));
    expect(states.some((layout) => layout.chip && !layout.chipBelow)).toBe(true);
    expect(states.some((layout) => layout.chip && layout.chipBelow)).toBe(true);
  });
});

describe("a tile's pieces from its card", () => {
  const container: Card = { ...card("stage"), title: "Build", journey: { state: "blocked", finished: false, relevance: "relevant" as const, foot: { words: "Due Feb 5, 2027", tone: "plain" }, owner: undefined, owners: "", body: { kind: "progress", done: 9, total: 12, badge: { flag: "overdue", tone: "bad" } }, here: { frontier: false, active: false, mine: false }, rank: undefined, current: false } };

  it("is the card's own title, state, progress, message and date, and the fact it is given", () => {
    const pieces = tilePiecesOf(container, "6 matches");
    expect(pieces).toMatchObject({ title: "Build", words: "9 of 12 done", meter: true, message: "overdue", date: "Due Feb 5, 2027", fact: "6 matches" });
    expect(pieces.chip).toBeDefined();
  });

  it("is only the title for a card that says nothing else", () => {
    expect(tilePiecesOf(card("loose"), undefined)).toMatchObject({ title: "loose", chip: undefined, words: undefined, meter: false, date: undefined, fact: undefined });
  });
});
