// The overlay's geometry over rectangles: what the scrim leaves open, what a scroller hides of a
// target, what room is left for the canvas around the callout, where the callout goes, and what is
// said of a target that is missing.
import { describe, expect, it } from "vitest";

import type { Rect } from "../ui/place.ts";
import { MISSING_MS, chooseCallout, clipTo, diagnose, dockRect, freeRoom, grow, holds, intersect, meets, scrimOf, strip, union, type CalloutInput } from "./track.ts";

const box = (left: number, top: number, width: number, height: number): Rect => ({ left, top, width, height });
const area = (rect: Rect): number => rect.width * rect.height;

describe("the scrim", () => {
  const viewport = { width: 1000, height: 700 };

  it("tiles the window around the hole: together with the hole its four rectangles are the whole window, and no press outside the hole misses them", () => {
    const hole = box(300, 200, 250, 120);
    const rects = scrimOf(hole, viewport);
    expect(rects.reduce((sum, rect) => sum + area(rect), 0) + area(hole)).toBe(viewport.width * viewport.height);
    for (const [x, y] of [[0, 0], [999, 699], [299, 250], [551, 250], [400, 199], [400, 321]] as const) {
      expect(rects.filter((rect) => x >= rect.left && x < rect.left + rect.width && y >= rect.top && y < rect.top + rect.height)).toHaveLength(1);
    }
    for (const [x, y] of [[300, 200], [549, 319], [400, 250]] as const) {
      expect(rects.some((rect) => x >= rect.left && x < rect.left + rect.width && y >= rect.top && y < rect.top + rect.height)).toBe(false);
    }
  });

  it("clamps a hole that reaches past the window, so nothing is drawn outside it and the rest still tiles", () => {
    const hole = box(-40, 650, 300, 200);
    const rects = scrimOf(hole, viewport);
    expect(rects.every((rect) => rect.width >= 0 && rect.height >= 0 && rect.left >= 0 && rect.top >= 0 && rect.left + rect.width <= viewport.width && rect.top + rect.height <= viewport.height)).toBe(true);
    const open = intersect(hole, box(0, 0, viewport.width, viewport.height));
    expect(rects.reduce((sum, rect) => sum + area(rect), 0) + area(open ?? box(0, 0, 0, 0))).toBe(viewport.width * viewport.height);
  });
});

describe("rectangles", () => {
  it("joins what is on the screen, and grows by a pad", () => {
    expect(union([box(10, 10, 20, 20), box(50, 40, 10, 10)])).toEqual(box(10, 10, 50, 40));
    expect(union([])).toBeUndefined();
    expect(grow(box(10, 10, 20, 20), 8)).toEqual(box(2, 2, 36, 36));
  });

  it("clips a target to the scrollers that hold it, and loses it when it is wholly scrolled out", () => {
    const scroller = box(0, 0, 400, 300);
    expect(clipTo(box(100, 250, 50, 100), [scroller])).toEqual(box(100, 250, 50, 50));
    expect(clipTo(box(100, 320, 50, 40), [scroller])).toBeUndefined();
    expect(clipTo(box(100, 250, 50, 100), [scroller, box(0, 0, 400, 270)])).toEqual(box(100, 250, 50, 20));
  });

  it("meets only where the rectangles overlap, not where they touch", () => {
    expect(meets(box(0, 0, 10, 10), box(10, 0, 10, 10))).toBe(false);
    expect(meets(box(0, 0, 10, 10), box(9, 9, 10, 10))).toBe(true);
    expect(holds(box(0, 0, 10, 10), 10, 5)).toBe(true);
    expect(holds(box(0, 0, 10, 10), 11, 5)).toBe(false);
  });
});

describe("the room a callout leaves", () => {
  const canvas = box(200, 50, 700, 600);

  it("reserves a strip on the callout's side, its size and gap deep", () => {
    expect(strip(canvas, "right", { width: 360, height: 200 }, 20)).toEqual(box(520, 50, 380, 600));
    expect(strip(canvas, "bottom", { width: 360, height: 200 }, 20)).toEqual(box(200, 430, 700, 220));
  });

  it("is the largest slab of the canvas the callout leaves", () => {
    const callout = box(520, 50, 380, 600);
    expect(freeRoom(canvas, callout)).toEqual(box(200, 50, 320, 600));
    // A callout over the canvas's middle leaves the larger of the sides.
    expect(freeRoom(canvas, box(300, 200, 200, 100))).toEqual(box(200, 300, 700, 350));
    // One that does not meet it takes nothing.
    expect(freeRoom(canvas, box(0, 0, 100, 100))).toEqual(canvas);
  });
});

describe("the callout's place", () => {
  const anchor = box(300, 280, 240, 100);
  const base: CalloutInput = { anchor, avoid: [anchor], size: { width: 360, height: 200 }, docked: { width: 640, height: 200 }, viewport: { width: 1280, height: 800 }, floor: 800, side: "right" };

  it("sits beside its anchor on a wide window, a gap from it, with the notch where the anchor's centre falls", () => {
    expect(chooseCallout(base)).toEqual({ side: "right", left: 540 + 16, top: 330 - 100, width: undefined, notchAt: 100 });
  });

  it("takes the other side when its own is full, and keeps clear of what the sentence names", () => {
    const middle = box(600, 280, 240, 100);
    const named = box(870, 200, 300, 300);
    expect(chooseCallout({ ...base, anchor: middle, avoid: [middle] })).toMatchObject({ side: "right", left: 856 });
    expect(chooseCallout({ ...base, anchor: middle, avoid: [middle, named] })).toMatchObject({ side: "left", left: 600 - 16 - 360 });
  });

  it("tries the other alignments before giving up a side", () => {
    // Centred beside the anchor it would cover the named box above it; lined up with the anchor's bottom it does not.
    const named = box(556, 150, 360, 100);
    const at = chooseCallout({ ...base, avoid: [anchor, named] });
    expect(at.side).toBe("right");
    expect(at.top).toBeGreaterThanOrEqual(250);
  });

  it("docks across the foot of the room above the sheet on a phone, a window too short for it, or an anchor in the sheet", () => {
    expect(chooseCallout({ ...base, viewport: { width: 390, height: 844 }, floor: 800 })).toMatchObject({ side: "dock", top: 800 - 12 - 200, left: 12, width: 366, notchAt: undefined });
    // Its height across the foot, not its height at its own width, sets where it sits.
    expect(chooseCallout({ ...base, docked: { width: 640, height: 120 }, viewport: { width: 390, height: 844 }, floor: 800 })).toMatchObject({ side: "dock", top: 800 - 12 - 120 });
    // A window the callout does not fit the height of docks; a wide, short one that holds it does not.
    expect(chooseCallout({ ...base, viewport: { width: 1280, height: 220 }, floor: 220 }).side).toBe("dock");
    const small = box(100, 150, 200, 100);
    expect(chooseCallout({ ...base, anchor: small, avoid: [small], viewport: { width: 844, height: 390 }, floor: 390 }).side).not.toBe("dock");
    const inSheet = box(300, 600, 240, 100);
    expect(chooseCallout({ ...base, anchor: inSheet, avoid: [inSheet], floor: 500 }).side).toBe("dock");
  });

  it("docks when no side leaves it clear, and when there is no anchor to sit beside", () => {
    expect(chooseCallout({ ...base, avoid: [anchor, box(0, 0, 1280, 800)] }).side).toBe("dock");
    expect(chooseCallout({ ...base, anchor: undefined, avoid: [] }).side).toBe("dock");
  });

  it("pushes a docked callout aside, or up to the head of the window, to keep clear of what it must not cover", () => {
    const dock = { ...base, viewport: { width: 1000, height: 600 }, floor: 600 };
    const rightOfCentre = box(700, 380, 280, 200);
    // The centred dock (180..820) would cover a box on the right; the same foot, pushed left, does not.
    const aside = chooseCallout({ ...dock, anchor: undefined, avoid: [rightOfCentre] });
    expect(aside).toMatchObject({ side: "dock", left: 12, bottomGap: 12 });
    // Boxes on both sides of the foot leave no room across it, so it goes to the head.
    const head = chooseCallout({ ...dock, anchor: undefined, avoid: [box(0, 380, 400, 220), box(600, 380, 400, 220)] });
    expect(head).toMatchObject({ side: "dock", top: 12, bottomGap: undefined });
  });

  it("keeps a docked callout's top at the margin where the room above the sheet is shorter than it, rather than hanging it from the foot", () => {
    const tight = chooseCallout({ ...base, docked: { width: 640, height: 300 }, viewport: { width: 390, height: 844 }, floor: 250 });
    expect(tight).toMatchObject({ side: "dock", top: 12, bottomGap: undefined });
  });

  it("caps a docked callout on a wide window and centres it", () => {
    expect(dockRect({ width: 360, height: 200 }, { width: 1000, height: 420 }, 420)).toEqual(box(180, 208, 640, 200));
  });
});

describe("a missing target", () => {
  it("is loading until the view is read, then closed when its pane is shut, and missing only once it has been absent for the time allowed", () => {
    expect(diagnose({ loaded: false, closed: false, absentFor: 10 * MISSING_MS })).toBe("loading");
    expect(diagnose({ loaded: true, closed: true, absentFor: 0 })).toBe("closed");
    expect(diagnose({ loaded: true, closed: false, absentFor: MISSING_MS - 1 })).toBe("loading");
    expect(diagnose({ loaded: true, closed: false, absentFor: MISSING_MS })).toBe("missing");
  });
});
