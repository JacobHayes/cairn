// C1: where the date lines run. Lines meeting at one side of a card take ports of their own,
// evenly spaced, and a route keeps out of the cards and stage frames it has no business in.
import { describe, expect, test } from "vitest";

import { dateRoutes, STUB_PX, type RouteCard } from "./dateRoutes.ts";
import type { Box } from "./EdgeLine.tsx";

const box = (x: number, y: number, width = 240, height = 96): Box => ({ x, y, width, height });
const cardOf = (key: string, parent?: string, container = false): RouteCard => ({ key, parent, container });

/** Whether a route's segment passes through the inside of a box. */
function crosses(route: readonly { x: number; y: number }[], inside: Box): boolean {
  return route.slice(1).some((to, at) => {
    const from = route[at];
    return from !== undefined && Math.max(from.x, to.x) > inside.x && Math.min(from.x, to.x) < inside.x + inside.width && Math.max(from.y, to.y) > inside.y && Math.min(from.y, to.y) < inside.y + inside.height;
  });
}

describe("date routes", () => {
  test("lines arriving at one side of a card enter at distinct, evenly spaced places", () => {
    const boxes = new Map([["target", box(600, 100)], ...[0, 1, 2].map((at): [string, Box] => [`s${String(at)}`, box(0, at * 150)])]);
    const lines = [0, 1, 2].map((at) => ({ from: `s${String(at)}`, to: "target" }));
    const routes = dateRoutes(lines, [...boxes.keys()].map((key) => cardOf(key)), boxes);
    const ends = routes.map((route) => route.at(-1));
    expect(new Set(ends.map((end) => end?.y)).size).toBe(3);
    const [a, b, c] = ends.map((end) => end?.y ?? 0).sort((x, y) => x - y) as [number, number, number];
    expect(b - a).toBeCloseTo(c - b, 6);
    for (const end of ends) {
      expect(end?.x).toBe(600);
    }
  });

  test("a route leaves and enters by a straight stub facing out of the card", () => {
    const boxes = new Map([["a", box(0, 0)], ["b", box(500, 300)]]);
    const [route] = dateRoutes([{ from: "a", to: "b" }], [cardOf("a"), cardOf("b")], boxes);
    expect(route?.[0]).toMatchObject({ x: 240 });
    expect(route?.[1]).toMatchObject({ y: route?.[0]?.y });
    expect((route?.[1]?.x ?? 0) - 240).toBeGreaterThanOrEqual(STUB_PX);
    expect(route?.at(-1)).toMatchObject({ x: 500 });
    expect(route?.at(-2)?.y).toBe(route?.at(-1)?.y);
  });

  test("a route keeps out of a card between its ends", () => {
    const boxes = new Map([["a", box(0, 0)], ["b", box(700, 0)], ["between", box(300, -20, 240, 140)]]);
    const [route] = dateRoutes([{ from: "a", to: "b" }], [...boxes.keys()].map((key) => cardOf(key)), boxes);
    expect(route?.length).toBeGreaterThan(2);
    expect(crosses(route ?? [], boxes.get("between") as Box)).toBe(false);
  });

});

describe("date routes and stage frames", () => {
  test("a route goes round a stage frame that holds neither end when a gutter exists", () => {
    const boxes = new Map([["a", box(0, 100)], ["b", box(900, 100)], ["frame", box(300, 60, 360, 180)], ["inside", box(320, 120, 240, 96)]]);
    const cards = [cardOf("a"), cardOf("b"), cardOf("frame", undefined, true), cardOf("inside", "frame")];
    const [route] = dateRoutes([{ from: "a", to: "b" }], cards, boxes);
    expect(crosses(route ?? [], boxes.get("frame") as Box)).toBe(false);
  });

  test("a route may leave and enter the frames that hold its ends", () => {
    const boxes = new Map([["f1", box(0, 0, 300, 160)], ["a", box(20, 40, 240, 96)], ["f2", box(500, 0, 300, 160)], ["b", box(520, 40, 240, 96)]]);
    const cards = [cardOf("f1", undefined, true), cardOf("a", "f1"), cardOf("f2", undefined, true), cardOf("b", "f2")];
    const [route] = dateRoutes([{ from: "a", to: "b" }], cards, boxes);
    expect(route?.[0]).toMatchObject({ x: 260 });
    expect(route?.at(-1)).toMatchObject({ x: 520 });
  });

  test("a line against the order of the columns runs from the left side of the first card to the right side of the second", () => {
    const boxes = new Map([["a", box(600, 0)], ["b", box(0, 0)]]);
    const [route] = dateRoutes([{ from: "a", to: "b" }], [cardOf("a"), cardOf("b")], boxes);
    expect(route?.[0]).toMatchObject({ x: 600 });
    expect(route?.at(-1)).toMatchObject({ x: 240 });
  });

  test("lines drawn together do not share a stretch while a free track exists", () => {
    const boxes = new Map([["a", box(0, 0)], ["b", box(600, 400)], ["c", box(0, 200)], ["d", box(600, 600)]]);
    const routes = dateRoutes([{ from: "a", to: "b" }, { from: "c", to: "d" }], [...boxes.keys()].map((key) => cardOf(key)), boxes);
    const verticals = routes.map((route) => route.slice(1).flatMap((to, at) => (route[at] !== undefined && route[at].x === to.x && route[at].y !== to.y ? [to.x] : [])));
    expect(verticals[0]?.some((x) => verticals[1]?.includes(x))).toBe(false);
  });

  test("no port is on the middle of a side, where a dependency meets the card, whatever the number of lines", () => {
    for (const count of [1, 2, 3, 4, 5]) {
      const sources = Array.from({ length: count }, (_, at) => `s${String(at)}`);
      const boxes = new Map<string, Box>([["t", box(600, 0)], ...sources.map((key, at): [string, Box] => [key, box(0, at * 150)])]);
      const routes = dateRoutes(sources.map((from) => ({ from, to: "t" })), [cardOf("t"), ...sources.map((key) => cardOf(key))], boxes);
      for (const route of routes) {
        expect(Math.abs((route.at(-1)?.y ?? 48) - 48)).toBeGreaterThanOrEqual(4);
      }
    }
  });

  test("a hundred lines at once all reach their ports", () => {
    const keys = Array.from({ length: 100 }, (_, at) => `c${String(at)}`);
    const boxes = new Map<string, Box>([["m", box(4000, 0)], ...keys.map((key, at): [string, Box] => [key, box((at % 10) * 336, Math.floor(at / 10) * 140)])]);
    const routes = dateRoutes(keys.map((key) => ({ from: key, to: "m" })), [...boxes.keys()].map((key) => cardOf(key)), boxes);
    expect(routes.every((route, at) => route[0]?.x === (boxes.get(keys[at] ?? "")?.x ?? 0) + 240 && route.at(-1)?.x === 4000)).toBe(true);
  });
});
