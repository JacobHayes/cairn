// The canvas's Safari pinch listener (5.10): a pinch zooms once, and not twice when a ctrl-key
// wheel arrives too (xyflow already zoomed that one).
import { expect, test } from "vitest";

import { LEGIBLE_ZOOM, firstFrame, listenPinch, roomOf, shiftInto, type Viewport } from "./gestures.ts";

test("a Safari pinch zooms once, and is left to the wheel when a ctrl-key wheel arrived since it began", () => {
  const root = Object.assign(new EventTarget(), { getBoundingClientRect: () => ({ left: 0, top: 0, width: 800, height: 600 }) }) as unknown as HTMLElement;
  let view: Viewport = { x: 0, y: 0, zoom: 0.5 };
  let sets = 0;
  listenPinch(root, {
    viewport: () => view,
    setViewport: (next) => {
      view = next;
      sets += 1;
    },
    minZoom: () => 0.1,
    maxZoom: () => 1.5,
  });
  const send = (type: string, extra: object = {}) => root.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), extra));
  send("gesturestart", { scale: 1 });
  send("gesturechange", { scale: 2 });
  expect([view.zoom, sets]).toEqual([1, 1]);
  send("gesturestart", { scale: 1 });
  send("wheel", { ctrlKey: true });
  send("gesturechange", { scale: 2 });
  expect(sets).toBe(1);
});

const PAD = { top: 10, left: 10, right: 10, bottom: 10 };

test("the first frame holds every changed card when they fit at a legible zoom, centred in the room", () => {
  const room = roomOf({ width: 420, height: 320 }, PAD, 100);
  const view = firstFrame([{ x: 0, y: 0, width: 100, height: 50 }, { x: 200, y: 100, width: 100, height: 50 }], room);
  expect(view?.zoom).toBeGreaterThanOrEqual(LEGIBLE_ZOOM);
  // Both ends are inside the room, with the same margin on each side.
  const [left, right] = [(view?.x ?? 0), (view?.x ?? 0) + 300 * (view?.zoom ?? 1)];
  expect(left).toBeGreaterThanOrEqual(room.left);
  expect(right).toBeLessThanOrEqual(room.right);
  expect(left - room.left).toBeCloseTo(room.right - right, 5);
});

test("when the changes do not fit at a legible zoom the first is framed alone, at the largest zoom the room holds it", () => {
  const room = roomOf({ width: 420, height: 320 }, PAD, 0);
  const view = firstFrame([{ x: 1000, y: 800, width: 100, height: 50 }, { x: 0, y: 0, width: 100, height: 50 }], room);
  expect(view?.zoom).toBe(1);
  const first = { left: 1000 * (view?.zoom ?? 0) + (view?.x ?? 0), top: 800 * (view?.zoom ?? 0) + (view?.y ?? 0) };
  expect(first.left).toBeGreaterThanOrEqual(room.left);
  expect(first.left + 100 * (view?.zoom ?? 0)).toBeLessThanOrEqual(room.right);
  expect(first.top).toBeGreaterThanOrEqual(room.top);
  expect(first.top + 50 * (view?.zoom ?? 0)).toBeLessThanOrEqual(room.bottom);
});

test("a span is moved into a room by the least move, and one longer than it starts at its start", () => {
  expect([shiftInto(20, 50, 10, 200), shiftInto(-5, 50, 10, 200), shiftInto(180, 50, 10, 200)]).toEqual([0, 15, -30]);
  expect(shiftInto(80, 500, 10, 200)).toBe(-70);
});

test("a first change taller than the room is framed at the legible zoom, from its top", () => {
  const room = roomOf({ width: 420, height: 320 }, PAD, 0);
  const view = firstFrame([{ x: 50, y: 100, width: 100, height: 2000 }, { x: 0, y: 0, width: 100, height: 50 }], room);
  expect(view?.zoom).toBe(LEGIBLE_ZOOM);
  expect(100 * LEGIBLE_ZOOM + (view?.y ?? 0)).toBe(room.top);
});

test("a later change that fits with the first at a legible zoom is framed with it, and one too far off is left to the rail", () => {
  const room = roomOf({ width: 420, height: 320 }, PAD, 0);
  const first = { x: 0, y: 0, width: 100, height: 50 };
  const near = { x: 150, y: 80, width: 100, height: 50 };
  const far = { x: 5000, y: 0, width: 100, height: 50 };
  const view = firstFrame([first, far, near], room);
  const zoom = view?.zoom ?? 0;
  expect(zoom).toBeGreaterThanOrEqual(LEGIBLE_ZOOM);
  // The near change's far corner is inside the room beside the first's near one.
  expect(250 * zoom + (view?.x ?? 0)).toBeLessThanOrEqual(room.right);
  expect(130 * zoom + (view?.y ?? 0)).toBeLessThanOrEqual(room.bottom);
  expect(0 * zoom + (view?.x ?? 0)).toBeGreaterThanOrEqual(room.left);
});
