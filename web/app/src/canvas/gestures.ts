// The canvas's gestures that xyflow does not handle. Chromium and Firefox report a
// trackpad pinch as a ctrl-key wheel, which xyflow zooms at the pointer. Safari may instead send
// only WebKit's `gesturestart`, `gesturechange` and `gestureend`: this listener zooms on those,
// unless a ctrl-key wheel arrived since the gesture began (xyflow already handled it, so zooming
// again would double it). It also stops Safari's own page zoom. Whether Safari sends wheels or
// gestures cannot be shown by a synthetic event, so no test covers the real pinch.

/** A viewport: the canvas's offset and zoom. */
export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

/** What the listener reads and moves. */
export interface PinchTarget {
  viewport(): Viewport;
  setViewport(viewport: Viewport): void;
  minZoom(): number;
  maxZoom(): number;
}

/** The viewport after zooming to `zoom` with the point `at` (in the canvas's own pixels) held still. */
export function zoomedAt(viewport: Viewport, at: { x: number; y: number }, zoom: number): Viewport {
  const ratio = zoom / viewport.zoom;
  return { x: at.x - (at.x - viewport.x) * ratio, y: at.y - (at.y - viewport.y) * ratio, zoom };
}

/** A WebKit gesture event: `scale` is the pinch's scale since it began. */
interface GestureLike extends Event {
  scale: number;
  clientX: number;
  clientY: number;
}

/** Listens for a Safari pinch on `root`; returns what takes the listeners off. */
export function listenPinch(root: HTMLElement, target: PinchTarget): () => void {
  let start = 1;
  let wheeled = false;
  const wheel = (event: WheelEvent) => {
    if (event.ctrlKey) {
      wheeled = true;
    }
  };
  const begin = (event: Event) => {
    event.preventDefault();
    wheeled = false;
    start = target.viewport().zoom;
  };
  const change = (event: Event) => {
    event.preventDefault();
    if (wheeled) {
      return;
    }
    const { scale, clientX, clientY } = event as GestureLike;
    const box = root.getBoundingClientRect();
    const viewport = target.viewport();
    const zoom = Math.min(target.maxZoom(), Math.max(target.minZoom(), start * scale));
    target.setViewport(zoomedAt(viewport, { x: (clientX || box.left + box.width / 2) - box.left, y: (clientY || box.top + box.height / 2) - box.top }, zoom));
  };
  const end = (event: Event) => {
    event.preventDefault();
  };
  root.addEventListener("wheel", wheel, { capture: true, passive: true });
  root.addEventListener("gesturestart", begin);
  root.addEventListener("gesturechange", change);
  root.addEventListener("gestureend", end);
  return () => {
    root.removeEventListener("wheel", wheel, { capture: true });
    root.removeEventListener("gesturestart", begin);
    root.removeEventListener("gesturechange", change);
    root.removeEventListener("gestureend", end);
  };
}

/** The zoom at which `bounds` fits a viewport of `size` with `pad` px kept free on each side. */
export function fitZoom(bounds: { width: number; height: number }, size: { width: number; height: number }, pad: { top: number; right: number; bottom: number; left: number }): number {
  if (bounds.width <= 0 || bounds.height <= 0) {
    return 1;
  }
  // A viewport smaller than its padding (a short tablet under a sheet) still fits at a tiny positive zoom, never a negative one.
  return Math.min(Math.max(1, size.width - pad.left - pad.right) / bounds.width, Math.max(1, size.height - pad.top - pad.bottom) / bounds.height);
}

/** The most the canvas zooms out: 0.1, or less when the whole graph needs it, so fit-all always fits. */
export function minZoomOf(fit: number): number {
  return Math.min(0.1, fit * 0.9);
}

/** Below this zoom a container's head is simplified: its title, and a stage's state chip. */
export const NEAR_ZOOM = 0.6;

/** The zoom a card's title stays readable at (about 10px of its 14px): a first fit never goes below it. */
export const LEGIBLE_ZOOM = 0.75;

/** A box on the canvas (flow coordinates, or px on screen where said). */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The part of the canvas a view may use: the lines `low` and `high` on each axis. */
export interface Room {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** The room a canvas of `size` leaves once `pad` is kept clear for the floating controls and `covered` px of its lower part lie under a sheet. */
export function roomOf(size: { width: number; height: number }, pad: { top: number; right: number; bottom: number; left: number }, covered: number): Room {
  return { left: pad.left, right: size.width - pad.right, top: pad.top, bottom: size.height - covered - pad.bottom };
}

/**
 * How far to move a span `size` long starting at `from` so it lies in `low..high`: by the least move (none when it is already whole in it), and a span longer than the room has its start at the room's start, so its head is read.
 */
export function shiftInto(from: number, size: number, low: number, high: number): number {
  if (size >= high - low) {
    return low - from;
  }
  return from < low ? low - from : from + size > high ? high - (from + size) : 0;
}

/** The viewport that puts `box` (flow coordinates) at `zoom` in the middle of `room`, or, where it is longer than the room on an axis, at its start. */
export function viewportAround(box: Box, zoom: number, room: Room): { x: number; y: number; zoom: number } {
  const place = (from: number, size: number, low: number, high: number) => (size >= high - low ? low : (low + high) / 2 - size / 2) - from;
  return { zoom, x: place(box.x * zoom, box.width * zoom, room.left, room.right), y: place(box.y * zoom, box.height * zoom, room.top, room.bottom) };
}

/** The zoom at which `box` fits `room`, at most 1. */
function zoomFitting(box: Box, room: Room): number {
  return Math.min(1, (room.right - room.left) / Math.max(1, box.width), (room.bottom - room.top) / Math.max(1, box.height));
}

/** The smallest box holding both. */
function unionOf(left: Box, right: Box): Box {
  const [x, y] = [Math.min(left.x, right.x), Math.min(left.y, right.y)];
  return { x, y, width: Math.max(left.x + left.width, right.x + right.width) - x, height: Math.max(left.y + left.height, right.y + right.height) - y };
}

/**
 * C14: the view a proposal's layer opens on. The boxes are the changed cards in the rail's order. The first is always framed; each later one is framed with it when all of them still fit `room` at or above `legible` (LEGIBLE_ZOOM unless the caller reads at a lower floor; never zoomed in past 1), so a change next to the first is in view with it, and the others, too far off to read together, are reached from the rail's rows. A first change alone is as large as the room holds it, never below LEGIBLE_ZOOM.
 */
export function firstFrame(boxes: readonly Box[], room: Room, legible: number = LEGIBLE_ZOOM): { x: number; y: number; zoom: number } | undefined {
  const first = boxes[0];
  if (first === undefined) {
    return undefined;
  }
  const framed = boxes.slice(1).reduce((held, box) => (zoomFitting(unionOf(held, box), room) >= legible ? unionOf(held, box) : held), first);
  return viewportAround(framed, Math.max(legible, zoomFitting(framed, room)), room);
}
