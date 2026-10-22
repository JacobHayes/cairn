// `EdgeLine`: one edge on the canvas (C1), drawn along the route ELK left room for (5.6): an
// orthogonal polyline with 4px corners, and at its target end the marker that says what it is: a
// closed arrowhead for a requirement, a hollow diamond for a condition gate, a bar for a stage
// opening. No words on the line; hovering or focusing one gives the
// sentence in a tip (GraphCanvas), which is also the edge's accessible name, and a click selects it. The line is steel, and an overlay's trace colours it. A date relationship
// (`DateEdgeLine`) is a different thing, drawn only for the selected node: a dashed ink line with an
// open arrowhead at its later end where the order is fixed.
import { BaseEdge, useStore, type Edge, type EdgeProps } from "@xyflow/react";

import type { Point } from "./layout.ts";
import { DATED, lineLook } from "./look.ts";
import type { DateLine, Line } from "./model.ts";
import type { CanvasOverlay } from "./overlay.ts";

export type LineData = {
  line: Line;
  /** Where ELK routed it, from source to target. */
  route: Point[] | undefined;
  overlay: CanvasOverlay | undefined;
  /** Its requirement is finished: the line has nothing left to wait on. */
  satisfied: boolean;
  /** A filter fades it. */
  faded: boolean;
};
export type LineEdge = Edge<LineData, "line">;

export type DateData = {
  line: DateLine;
  /** Where it runs on the canvas, from its `from` card to its `to` card. */
  route: Point[];
};
export type DateEdge = Edge<DateData, "dates">;

/** The corner radius of a route (px), and the markers' length and half-width (px). */
export const CORNER_PX = 4;
export const MARKER_LENGTH_PX = 9;
const MARKER_HALF_PX = 4.5;

/** The marker's size at zoom `zoom`, as a multiple of its layout size: it grows as the graph shrinks (not to a hairline at fit) and shrinks as it zooms in (not huge), while the strokes stay constant on screen. */
export function markerScale(zoom: number): number {
  return Math.min(2.4, Math.max(0.6, zoom ** -0.75));
}

/** The unit vector from `from` to `to`; a point is its own direction when they meet. */
function direction(from: Point, to: Point): Point {
  const length = Math.hypot(to.x - from.x, to.y - from.y);
  return length === 0 ? { x: 1, y: 0 } : { x: (to.x - from.x) / length, y: (to.y - from.y) / length };
}

/** `route` with its last `trim` px removed, so a marker can sit where the line ends. */
function trimmed(route: Point[], trim: number): Point[] {
  const last = route.at(-1);
  const before = route.at(-2);
  if (last === undefined || before === undefined || trim === 0) {
    return route;
  }
  const length = Math.hypot(last.x - before.x, last.y - before.y);
  const way = direction(before, last);
  const cut = Math.min(trim, length);
  return [...route.slice(0, -1), { x: last.x - way.x * cut, y: last.y - way.y * cut }];
}

/** A polyline as an SVG path with each corner rounded to at most `radius`. */
export function polyline(points: Point[], radius = CORNER_PX): string {
  const [first, ...rest] = points;
  if (first === undefined) {
    return "";
  }
  let path = `M${String(first.x)} ${String(first.y)}`;
  rest.forEach((point, index) => {
    const before = points[index];
    const after = rest[index + 1];
    if (before === undefined || after === undefined) {
      path += ` L${String(point.x)} ${String(point.y)}`;
      return;
    }
    const [inward, outward] = [direction(point, before), direction(point, after)];
    const reach = Math.min(radius, Math.hypot(before.x - point.x, before.y - point.y) / 2, Math.hypot(after.x - point.x, after.y - point.y) / 2);
    path += ` L${String(point.x + inward.x * reach)} ${String(point.y + inward.y * reach)} Q${String(point.x)} ${String(point.y)} ${String(point.x + outward.x * reach)} ${String(point.y + outward.y * reach)}`;
  });
  return path;
}

/** The marker's outline at `end`, pointing along `way`: its tip at the end of the route. */
function markerPath(kind: "arrow" | "diamond" | "bar" | "open", end: Point, way: Point, scale: number): string {
  const [length, half] = [MARKER_LENGTH_PX * scale, MARKER_HALF_PX * scale];
  const across = { x: -way.y, y: way.x };
  const at = (along: number, side: number) => `${String(end.x - way.x * along + across.x * side)} ${String(end.y - way.y * along + across.y * side)}`;
  switch (kind) {
    case "arrow":
      return `M${at(0, 0)} L${at(length, half)} L${at(length, -half)} Z`;
    case "diamond":
      return `M${at(0, 0)} L${at(length / 2, half)} L${at(length, 0)} L${at(length / 2, -half)} Z`;
    case "bar":
      return `M${at(1.5, half + 1)} L${at(1.5, -half - 1)}`;
    case "open":
      return `M${at(length, half)} L${at(0, 0)} L${at(length, -half)}`;
  }
}

/** How far a marker reaches back from the route's end (px): the line stops there. */
function reach(kind: "arrow" | "diamond" | "bar" | "open" | "none", scale: number): number {
  return kind === "arrow" || kind === "diamond" ? MARKER_LENGTH_PX * scale : 0;
}

/** A card's box on the canvas. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function EdgeLine({ id, data, selected }: EdgeProps<LineEdge>) {
  // Rounded, so the lines redraw when the size changes visibly, not on every zoom tick.
  const scale = useStore((state) => Math.round(markerScale(state.transform[2]) * 10) / 10);
  if (data === undefined || data.route === undefined || data.route.length < 2) {
    return null;
  }
  const { line, route, overlay, satisfied, faded } = data;
  const look = lineLook(line, { overlay, satisfied, faded });
  const end = route.at(-1);
  const before = route.at(-2);
  const classes = [...look.classes, selected === true ? "line-selected" : ""].filter(Boolean).join(" ");
  return (
    <g
      data-testid="edge-line"
      data-from={line.from}
      data-to={line.to}
      data-kind={line.kind}
      data-dash={look.dash ?? "solid"}
      data-marker={look.marker}
      className={classes}
    >
      <BaseEdge id={id} path={polyline(trimmed(route, reach(look.marker, scale)))} />
      {end === undefined || before === undefined ? null : (
        <path className="line-marker" d={markerPath(look.marker, end, direction(before, end), scale)} />
      )}
      {line.count > 1 && end !== undefined ? (
        <text className="line-count" x={end.x - MARKER_LENGTH_PX * scale - 4} y={end.y - 6} textAnchor="end">
          {`\u00d7${String(line.count)}`}
        </text>
      ) : null}
    </g>
  );
}

/** C1: one date relationship, drawn for the selected node: dashed ink, an open arrowhead at the later end when the order is fixed. */
export function DateEdgeLine({ id, data }: EdgeProps<DateEdge>) {
  const scale = useStore((state) => Math.round(markerScale(state.transform[2]) * 10) / 10);
  if (data === undefined || data.route.length < 2) {
    return null;
  }
  const { line, route } = data;
  const end = route.at(-1);
  const before = route.at(-2);
  const marked = line.orders && end !== undefined && before !== undefined;
  return (
    <g data-testid="date-line" data-from={line.from} data-to={line.to} data-orders={line.orders ? "" : undefined} data-kind="dates" data-dash={DATED} data-marker={marked ? "open" : "none"} className="line line-dates" role="img" aria-label={line.sentence}>
      <BaseEdge id={id} path={polyline(route)} />
      {marked ? <path className="line-marker" d={markerPath("open", end, direction(before, end), scale)} /> : null}
    </g>
  );
}
