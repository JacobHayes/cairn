// C1: where a date line runs. Lines drawn together take lanes, so none lies on another, and a
// line between cards in one column keeps out of the cards between them.
import { describe, expect, test } from "vitest";

import { elbow, type Box } from "./EdgeLine.tsx";

const box = (x: number, y: number, width = 170, height = 70): Box => ({ x, y, width, height });

/** Whether a route's segment passes through the inside of a box. */
function crosses(route: readonly { x: number; y: number }[], inside: Box): boolean {
  return route.slice(1).some((to, at) => {
    const from = route[at];
    if (from === undefined) {
      return false;
    }
    const [x1, x2] = [Math.min(from.x, to.x), Math.max(from.x, to.x)];
    const [y1, y2] = [Math.min(from.y, to.y), Math.max(from.y, to.y)];
    return x2 > inside.x && x1 < inside.x + inside.width && y2 > inside.y && y1 < inside.y + inside.height;
  });
}

describe("elbow", () => {
  test("cards in one column are joined down the gutter, outside the cards between them", () => {
    const [top, middle, bottom] = [box(100, 0), box(100, 150), box(100, 300)];
    const route = elbow(top, bottom, { index: 0, lanes: 1 }, [top, middle, bottom]);
    expect(crosses(route, middle)).toBe(false);
    expect(route[0]).toMatchObject({ x: 100 });
    expect(route.at(-1)).toMatchObject({ x: 100 });
  });

  test("cards with a gap between them are joined across it, from the facing sides", () => {
    const route = elbow(box(0, 0), box(300, 200));
    expect(route[0]?.x).toBe(170);
    expect(route.at(-1)?.x).toBe(300);
  });

  test("cards with a gap between them are joined clear of the cards in the gap", () => {
    const [from, to] = [box(0, 0), box(500, 200)];
    const between = box(200, 20, 100, 60);
    const route = elbow(from, to, { index: 0, lanes: 1 }, [from, to, between]);
    expect(crosses(route, between)).toBe(false);
    expect(route[0]?.x).toBe(170);
    expect(route.at(-1)?.x).toBe(500);
  });

  test("the same holds when the first card is the one on the right", () => {
    const [from, to] = [box(500, 200), box(0, 0)];
    const between = box(200, 20, 100, 60);
    const route = elbow(from, to, { index: 0, lanes: 1 }, [from, to, between]);
    expect(crosses(route, between)).toBe(false);
  });

  test("lines drawn together run in lanes of their own", () => {
    const [from, to] = [box(0, 0), box(300, 200)];
    const middles = [0, 1, 2].map((index) => elbow(from, to, { index, lanes: 3 })[1]?.x);
    expect(new Set(middles).size).toBe(3);
    const gutters = [0, 1, 2].map((index) => elbow(box(100, 0), box(100, 300), { index, lanes: 3 })[1]?.x);
    expect(new Set(gutters).size).toBe(3);
  });
});
