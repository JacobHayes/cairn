// Where a floating box goes beside the thing it describes, inside the room it may use. One
// arithmetic for every floating surface (the tooltip, the keys preview, the sync popover, the
// context menu, the dropdown menus): put it on a side, flip to the other side when this one
// lacks the room and the other has more, align it on the other axis, then slide it back inside
// the bounds. Pure: the caller measures and applies, and everything is in viewport coordinates.

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export type Side = "top" | "bottom" | "left" | "right";
export type Align = "start" | "center" | "end";

export interface PlaceOptions {
  /** The side of the anchor the box prefers. */
  side: Side;
  /** How the box lines up with the anchor along the other axis: its leading edge, centre or trailing edge. */
  align: Align;
  /** The space between the anchor and the box (px). */
  gap: number;
  /** The space the box keeps from the bounds' edges (px). */
  margin: number;
}

export interface Placement {
  left: number;
  top: number;
  /** The side the box ended up on (the preferred one, or its opposite after a flip). */
  side: Side;
  /** The room the box has on that side, inside the margin (px): what a scrolling body may be capped to. */
  room: number;
}

const OPPOSITE: Record<Side, Side> = { top: "bottom", bottom: "top", left: "right", right: "left" };

/** The room a box has on `side` of the anchor, inside the bounds less the margin. */
function roomOn(side: Side, anchor: Rect, bounds: Rect, gap: number, margin: number): number {
  switch (side) {
    case "bottom":
      return bounds.top + bounds.height - margin - (anchor.top + anchor.height + gap);
    case "top":
      return anchor.top - gap - (bounds.top + margin);
    case "right":
      return bounds.left + bounds.width - margin - (anchor.left + anchor.width + gap);
    case "left":
      return anchor.left - gap - (bounds.left + margin);
  }
}

/** Where a box of `size` goes beside `anchor`, inside `bounds`. */
export function place(anchor: Rect, size: { width: number; height: number }, bounds: Rect, options: PlaceOptions): Placement {
  const { gap, margin } = options;
  const vertical = options.side === "top" || options.side === "bottom";
  const main = vertical ? size.height : size.width;
  const room = roomOn(options.side, anchor, bounds, gap, margin);
  const other = roomOn(OPPOSITE[options.side], anchor, bounds, gap, margin);
  const side = main > room && other > room ? OPPOSITE[options.side] : options.side;
  // Along the main axis: against the anchor, `gap` away.
  const mainAt = (() => {
    switch (side) {
      case "bottom":
        return anchor.top + anchor.height + gap;
      case "top":
        return anchor.top - gap - size.height;
      case "right":
        return anchor.left + anchor.width + gap;
      case "left":
        return anchor.left - gap - size.width;
    }
  })();
  // Along the other axis: lined up with the anchor.
  const [start, span, own] = vertical ? [anchor.left, anchor.width, size.width] : [anchor.top, anchor.height, size.height];
  const crossAt = options.align === "start" ? start : options.align === "end" ? start + span - own : start + (span - own) / 2;
  const [left, top] = vertical ? [crossAt, mainAt] : [mainAt, crossAt];
  return {
    left: slide(left, size.width, bounds.left, bounds.width, margin),
    top: slide(top, size.height, bounds.top, bounds.height, margin),
    side,
    room: side === options.side ? room : other,
  };
}

/** `at` moved the least that keeps `length` inside the span less the margin; a box too long for it pins to the leading margin. */
function slide(at: number, length: number, from: number, span: number, margin: number): number {
  const lead = from + margin;
  const trail = from + span - margin - length;
  return trail < lead ? lead : Math.min(Math.max(at, lead), trail);
}

/** The window, as bounds. */
export function windowBounds(): Rect {
  return { left: 0, top: 0, width: document.documentElement.clientWidth, height: globalThis.innerHeight };
}

/** A pointer as an anchor: the arrow cursor's footprint below-right of the hot spot (12 x 20). */
export function pointerRect(x: number, y: number): Rect {
  return { left: x, top: y, width: 12, height: 20 };
}
