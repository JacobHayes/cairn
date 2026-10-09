// The canvas's gestures that xyflow does not handle (5.10). Chromium and Firefox report a
// trackpad pinch as a ctrl-key wheel, which xyflow zooms at the pointer. Safari may instead send
// only WebKit's `gesturestart`, `gesturechange` and `gestureend`: this listener zooms on those,
// unless a ctrl-key wheel arrived since the gesture began (xyflow already handled it, so zooming
// again would double it). It also stops Safari's own page zoom. Whether Safari sends wheels or
// gestures cannot be shown by a synthetic event; the proof records the real pinch as unchecked.

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
  return Math.min((size.width - pad.left - pad.right) / bounds.width, (size.height - pad.top - pad.bottom) / bounds.height);
}

/** The most the canvas zooms out (5.6): 0.1, or less when the whole graph needs it, so fit-all always fits. */
export function minZoomOf(fit: number): number {
  return Math.min(0.1, fit * 0.9);
}

/** The zoom band a card draws for (5.4): near, mid or far. */
export type Band = "near" | "mid" | "far";

export const NEAR_ZOOM = 0.6;
export const MID_ZOOM = 0.3;

export function bandOf(zoom: number): Band {
  return zoom >= NEAR_ZOOM ? "near" : zoom >= MID_ZOOM ? "mid" : "far";
}
