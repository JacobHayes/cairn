// PRD: C20.
// What the tour's overlay measures and decides from rectangles: the hole a spotlight cuts, the
// four rectangles of dimming around it, where the callout goes, what room the canvas or a scroller
// is asked to keep for the callout, and why a target is missing. The arithmetic is pure (the unit
// tests give it rectangles); the readers at the end are the only part that touches the page.
//
// The overlay does not subscribe to what moves a target (a canvas pan, a scroll, a sheet's drag, a
// disclosure opening, a pane mounting). It reads the target's rectangle once per animation frame
// and acts on a change, which covers all of them with one mechanism and cannot miss a new one.
import { place, type Align, type Rect, type Side } from "../ui/place.ts";

/** The space between a target and the hole's edge (px). */
export const HOLE_PAD = 8;

/** The space between the hole and the callout beside it (px). */
export const CALLOUT_GAP = 16;

/** How long a target may be missing, with the view loaded and the pane open, before the step says its control is not on the screen (ms). */
export const MISSING_MS = 2000;

/** How many animation frames a state's targets hold still before it is settled and drawn. */
export const SETTLE_FRAMES = 5;

export interface Size {
  width: number;
  height: number;
}

const right = (rect: Rect): number => rect.left + rect.width;
const bottom = (rect: Rect): number => rect.top + rect.height;

/** The smallest rectangle holding all of `rects`, or none for none. */
export function union(rects: readonly Rect[]): Rect | undefined {
  const [first, ...rest] = rects;
  if (first === undefined) {
    return undefined;
  }
  const [left, top] = [Math.min(first.left, ...rest.map((each) => each.left)), Math.min(first.top, ...rest.map((each) => each.top))];
  return { left, top, width: Math.max(right(first), ...rest.map(right)) - left, height: Math.max(bottom(first), ...rest.map(bottom)) - top };
}

/** `rect` grown by `by` on every side. */
export function grow(rect: Rect, by: number): Rect {
  return { left: rect.left - by, top: rect.top - by, width: rect.width + 2 * by, height: rect.height + 2 * by };
}

/** What two rectangles share, or none when they only touch or do not meet. */
export function intersect(a: Rect, b: Rect): Rect | undefined {
  const [left, top] = [Math.max(a.left, b.left), Math.max(a.top, b.top)];
  const [farRight, farBottom] = [Math.min(right(a), right(b)), Math.min(bottom(a), bottom(b))];
  return farRight > left && farBottom > top ? { left, top, width: farRight - left, height: farBottom - top } : undefined;
}

export const meets = (a: Rect, b: Rect): boolean => intersect(a, b) !== undefined;

/** Whether the point is inside the rectangle. */
export function holds(rect: Rect, x: number, y: number): boolean {
  return x >= rect.left && x <= right(rect) && y >= rect.top && y <= bottom(rect);
}

/**
 * The four rectangles that dim everything but `hole` (above it, below it, and the two sides
 * between): together with the hole they tile the viewport exactly, so a press anywhere but in the
 * hole lands on one of them. The hole is clamped into the viewport first.
 */
export function scrimOf(hole: Rect, viewport: Size): Rect[] {
  const [left, top] = [Math.min(Math.max(hole.left, 0), viewport.width), Math.min(Math.max(hole.top, 0), viewport.height)];
  const [farRight, farBottom] = [Math.min(Math.max(right(hole), left), viewport.width), Math.min(Math.max(bottom(hole), top), viewport.height)];
  return [
    { left: 0, top: 0, width: viewport.width, height: top },
    { left: 0, top: farBottom, width: viewport.width, height: viewport.height - farBottom },
    { left: 0, top, width: left, height: farBottom - top },
    { left: farRight, top, width: viewport.width - farRight, height: farBottom - top },
  ];
}

/** `rect` as far as it shows inside every box that clips it (a scroller, the canvas), or none when nothing of it does. */
export function clipTo(rect: Rect, clips: readonly Rect[]): Rect | undefined {
  return clips.reduce<Rect | undefined>((shown, clip) => (shown === undefined ? undefined : intersect(shown, clip)), rect);
}

/** The strip a callout of `size` takes against one side of `region`, `gap` and all. */
export function strip(region: Rect, side: Side, size: Size, gap: number): Rect {
  switch (side) {
    case "bottom":
      return { left: region.left, top: bottom(region) - size.height - gap, width: region.width, height: size.height + gap };
    case "top":
      return { left: region.left, top: region.top, width: region.width, height: size.height + gap };
    case "right":
      return { left: right(region) - size.width - gap, top: region.top, width: size.width + gap, height: region.height };
    case "left":
      return { left: region.left, top: region.top, width: size.width + gap, height: region.height };
  }
}

/** The largest of the four slabs of `region` that `taken` leaves (all of it when they do not meet): the room a callout leaves a target. */
export function freeRoom(region: Rect, taken: Rect): Rect {
  const cut = intersect(region, taken);
  if (cut === undefined) {
    return region;
  }
  const slabs: Rect[] = [
    { left: region.left, top: region.top, width: region.width, height: cut.top - region.top },
    { left: region.left, top: bottom(cut), width: region.width, height: bottom(region) - bottom(cut) },
    { left: region.left, top: region.top, width: cut.left - region.left, height: region.height },
    { left: right(cut), top: region.top, width: right(region) - right(cut), height: region.height },
  ];
  return slabs.reduce((best, slab) => (slab.width * slab.height > best.width * best.height ? slab : best));
}

/** Where the callout goes, and what it is drawn against. */
export interface Callout {
  /** Free beside the anchor: `place()`'s answer. Docked: across the foot of the room above the sheet. */
  left: number;
  top: number;
  width: number | undefined;
  side: Side | "dock";
  /** Along the near edge, where the pointer notch goes; none when docked. */
  notchAt: number | undefined;
  /** Docked: the gap under it to the window's foot, which keeps it on the sheet's edge whatever its height at its stretched width. */
  bottomGap: number | undefined;
}

export interface CalloutInput {
  /** What the callout sits beside; none when there is nothing on the screen to sit beside. */
  anchor: Rect | undefined;
  /** What the callout may not cover: the anchor, and whatever the step's sentence names. */
  avoid: readonly Rect[];
  size: Size;
  /** Its size once docked: stretched across the foot, so usually wider and shorter than `size`. */
  docked: Size;
  viewport: Size;
  /** The top of the sheet when one lies over the window's foot (below 1100px), else the window's bottom. */
  floor: number;
  /** The side of the anchor it prefers. */
  side: Side;
}

/** A window too narrow for a callout beside its target: the callout is docked at once. A short one is not (a phone on its side has the width), but still docks when the callout does not fit its height. */
export const isNarrow = (viewport: Size): boolean => viewport.width < 720;

/** The distance docked callouts keep from the window's edge (px), and the widest one gets (a tablet's window is wide). */
export const DOCK_MARGIN = 12;
export const DOCK_MAX = 640;

/** The space a callout keeps from the window's edge (px). */
export const EDGE_MARGIN = 12;

/** How wide a docked callout is in a window of `viewport`. */
export const dockWidth = (viewport: Size): number => Math.min(viewport.width - 2 * DOCK_MARGIN, DOCK_MAX);

/** Where a docked callout of `size` is: across the foot of the room above the sheet, centred. */
export function dockRect(size: Size, viewport: Size, floor: number): Rect {
  const width = dockWidth(viewport);
  return { left: (viewport.width - width) / 2, top: Math.max(DOCK_MARGIN, floor - DOCK_MARGIN - size.height), width, height: size.height };
}

const OPPOSITE: Record<Side, Side> = { top: "bottom", bottom: "top", left: "right", right: "left" };

/** How the callout lines up along the anchor: centred on a small anchor, at its leading edge on a large one (a notch pointing into the middle of a tall hole would have the callout far from its top). */
export function alignFor(anchor: Rect, side: Side): Align {
  const along = side === "left" || side === "right" ? anchor.height : anchor.width;
  return along > 260 ? "start" : "center";
}

/**
 * The callout's place: beside its anchor on its preferred side, or the opposite, lined up the way
 * that suits the anchor first and the others after, the first of those that covers nothing it must
 * keep clear; else docked across the foot of the room above the sheet, or across its head when the foot would cover what it must keep clear. A phone's window, one too short for it, an anchor with nothing on the screen, and an anchor in the sheet dock at once.
 */
export function chooseCallout({ anchor, avoid, size, docked, viewport, floor, side }: CalloutInput): Callout {
  const sideBySide = !isNarrow(viewport) && anchor !== undefined && size.width > 0 && size.height + 2 * EDGE_MARGIN <= floor && anchor.top + anchor.height / 2 <= floor;
  if (sideBySide) {
    const bounds: Rect = { left: 0, top: 0, width: viewport.width, height: floor };
    const aligns = [...new Set<Align>([alignFor(anchor, side), "center", "start", "end"])];
    for (const each of [side, OPPOSITE[side]]) {
      for (const align of aligns) {
        const placed = place(anchor, size, bounds, { side: each, align, gap: CALLOUT_GAP, margin: EDGE_MARGIN });
        if (placed.side === each && !avoid.some((keep) => meets({ left: placed.left, top: placed.top, ...size }, keep))) {
          return { left: placed.left, top: placed.top, width: undefined, side: each, notchAt: placed.anchorOffset, bottomGap: undefined };
        }
      }
    }
  }
  const dock = dockRect(docked, viewport, floor);
  // Across the foot, centred or pushed to either side of what it must keep clear; or, where none of those does, across the head of the window.
  const head = { ...dock, top: DOCK_MARGIN };
  const sides = [dock.left, DOCK_MARGIN, viewport.width - DOCK_MARGIN - dock.width];
  const foot = sides.map((left) => ({ ...dock, left }));
  const clear = (box: Rect) => !avoid.some((keep) => meets(box, keep));
  // Where the window is too short to hold the callout above the floor, its top is kept at the margin rather than hung from the foot.
  const bottomGap = dock.top > DOCK_MARGIN ? viewport.height - floor + DOCK_MARGIN : undefined;
  const at = foot.find(clear);
  if (at !== undefined) {
    return { left: at.left, top: at.top, width: at.width, side: "dock", notchAt: undefined, bottomGap };
  }
  const high = sides.map((left) => ({ ...head, left })).find(clear);
  if (high !== undefined) {
    return { left: high.left, top: high.top, width: high.width, side: "dock", notchAt: undefined, bottomGap: undefined };
  }
  return { left: dock.left, top: dock.top, width: dock.width, side: "dock", notchAt: undefined, bottomGap };
}

/** Why a target has no match: the view is still loading, the pane it lives in is closed, or it is missing and will not come. */
export type Absence = "loading" | "closed" | "missing";

export interface AbsenceInput {
  /** The practice journey is read and drawn. */
  loaded: boolean;
  /** The sheet is at its peek, or the pane that holds the target is shut. */
  closed: boolean;
  /** How long the target has been missing (ms). */
  absentFor: number;
}

/** What to tell a person about a missing target. */
export function diagnose({ loaded, closed, absentFor }: AbsenceInput): Absence {
  if (!loaded) {
    return "loading";
  }
  if (closed) {
    return "closed";
  }
  return absentFor >= MISSING_MS ? "missing" : "loading";
}

// ---- the page's side ---------------------------------------------------------------------------

const rectOf = (element: Element): Rect => {
  const box = element.getBoundingClientRect();
  return { left: box.left, top: box.top, width: box.width, height: box.height };
};

/** Whether the element is there to be pointed at: drawn, not hidden, and not inert (the sheet at its peek). */
function usable(element: Element): boolean {
  const box = element.getBoundingClientRect();
  return element.isConnected && element.closest("[inert]") === null && element.checkVisibility({ visibilityProperty: true }) && box.width > 0 && box.height > 0;
}

/** The boxes that clip `element`: every ancestor that scrolls or hides its overflow. */
function clipsOf(element: Element): Rect[] {
  const clips: Rect[] = [];
  for (let each = element.parentElement; each !== null && each !== document.body && each !== document.documentElement; each = each.parentElement) {
    const style = getComputedStyle(each);
    if (style.overflowX !== "visible" || style.overflowY !== "visible") {
      clips.push(rectOf(each));
    }
  }
  return clips;
}

/** What a set of selectors matches on the page now. */
export interface Matched {
  /** The usable matches, in document order. */
  elements: HTMLElement[];
  /** The part of the matches' bounding box that shows, or none when no match does (none matched, or all are scrolled out of their scroller). */
  rect: Rect | undefined;
  /** Some usable match is wholly out of view in its scroller. */
  scrolledOut: boolean;
}

/** The live matches of `selectors` (each scoped to the live surface by its author): usable ones only, their visible parts joined. */
export function match(selectors: readonly string[]): Matched {
  const elements = selectors.flatMap((selector) => [...document.querySelectorAll<HTMLElement>(selector)]).filter(usable);
  const shown = elements.flatMap((element) => {
    const part = clipTo(rectOf(element), clipsOf(element));
    return part === undefined ? [] : [part];
  });
  return { elements, rect: union(shown), scrolledOut: elements.length > 0 && shown.length === 0 };
}

/** The parts of each usable match of `selectors` that show. */
export function visibleRects(selectors: readonly string[]): Rect[] {
  return selectors.flatMap((selector) => [...document.querySelectorAll(selector)]).filter(usable).flatMap((element) => {
    const part = clipTo(rectOf(element), clipsOf(element));
    return part === undefined ? [] : [part];
  });
}

/** The nearest ancestor of `element` that scrolls vertically, if one does. */
export function scrollerOf(element: HTMLElement): HTMLElement | undefined {
  for (let each = element.parentElement; each !== null; each = each.parentElement) {
    const { overflowY } = getComputedStyle(each);
    if ((overflowY === "auto" || overflowY === "scroll") && each.scrollHeight > each.clientHeight) {
      return each;
    }
  }
  return undefined;
}

/** The window as a rectangle. */
export function viewportRect(): Rect {
  return { left: 0, top: 0, width: document.documentElement.clientWidth, height: globalThis.innerHeight };
}

/** The rectangle of the element, in the window's coordinates. */
export { rectOf };

/** Whether two rectangles are the same to within half a pixel (a frame's rounding), or both absent. */
export function sameRect(a: Rect | undefined, b: Rect | undefined): boolean {
  if (a === undefined || b === undefined) {
    return a === b;
  }
  return Math.abs(a.left - b.left) < 0.5 && Math.abs(a.top - b.top) < 0.5 && Math.abs(a.width - b.width) < 0.5 && Math.abs(a.height - b.height) < 0.5;
}
