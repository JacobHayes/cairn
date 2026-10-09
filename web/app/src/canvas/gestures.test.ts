// The canvas's Safari pinch listener (5.10): a pinch zooms once, and not twice when a ctrl-key
// wheel arrives too (xyflow already zoomed that one).
import { expect, test } from "vitest";

import { listenPinch, type Viewport } from "./gestures.ts";

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
