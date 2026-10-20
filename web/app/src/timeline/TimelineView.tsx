// C13: the timeline. Rows are the dated nodes the journey's level draws (Depth's step, the
// Filter and the search every view shares), under the containment tree, folded by the ladder;
// each has its date in plain words beside a track on one shared time axis with today and the end
// anchor (the `final` milestone) drawn across it. A route's timeline (timeline/RouteTimeline.tsx)
// draws the same chart over days from its start: `TimelineChart` takes the rows, the axis it lies on
// and how a row is titled, so the journey's and the route's are one implementation. Selecting a row shows its float and the lines
// to what it needs and unblocks (a shortfall's chain in error ink) and fades the rest; Frontier
// fades what it does not light, and Depth's Fade what is below the depth. Two-finger horizontal scroll pans time, a pinch
// zooms it, vertical scroll moves the rows (the workspace's own). Nodes with no date gather in
// a folded list. A row's title opens the node's detail (5.1) beside the timeline, which
// explains its dates (F7).
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { useLocation, useNavigate } from "react-router";

import { useFolds, useLevel, useProjected } from "../canvas/hooks.ts";

import { titleOf, type Ready } from "../detail/model.ts";
import { NodeLink } from "../detail/parts.tsx";
import { useNodeAddresses, withParam } from "../journeys/address.ts";

import type { ViewState } from "../journeys/view.ts";
import { EmptyPanel, journeySubject, NothingMatches, ViewBar } from "../screens/ViewControls.tsx";
import { Button, Segmented } from "../ui/kit.tsx";
import { TreeFold, TreeGuides } from "../ui/Tree.tsx";
import { dateWords } from "../status/when.ts";
import { axisOf, calendarSpec, dateOf, dayWords, positionOf, rangeWindow, RANGES, type Axis, type AxisSpec, type Range, type Window } from "./model.ts";
import { chartOf, type Chart, type ChartRow, type Dependency } from "./rows.ts";
import "./timeline.css";
import { Failed } from "../ui/Failed.tsx";
import { useViewLevel } from "../canvas/viewLevel.ts";
import { useLayer } from "../proposals/layer.ts";
import { DIFF_LABELS, type DiffMark } from "../proposals/model.ts";

/** The height of one row's track, and of the axis above the rows (px). */
const ROW_PX = 56;
const AXIS_PX = 40;

/** A diamond's half width, and a bar's half height (px). */
const DIAMOND_PX = 8;
const BAR_PX = 7;

/** The least a bar is drawn wide, as a fraction of the axis, so a one-day bar can still be seen. */
const BAR_WIDTH_MIN = 0.004;

/** The most ticks the axis names when its width is not known; with more, every second or third one is. */
const TICK_LABEL_COUNT_MAX = 7;

/** The least room (px) a tick's name takes along the axis, and what a name keeps clear of the right edge and of the end line. */
const TICK_LABEL_PX = 68;
const TICK_LABEL_EDGE_PX = 48;

/** The fewest and most days a pinch leaves in view. */
const SPAN_DAYS_MIN = 7;
const SPAN_DAYS_MAX = 3650;

/** Past this fraction of the width, a label on the axis reads leftward, so it stays on it. */
const LABEL_FLIP_AT = 0.75;

/** What a character of the axis's 11px mono labels takes (px), and the gap two labels keep between them. */
const LABEL_CHAR_PX = 6.6;
const LABEL_GAP_PX = 8;

/** The stretch (px) a label of `words` takes along the axis from `x`, reading toward `anchor`'s side. */
function spanOf(words: string, x: number, anchor: "start" | "end"): [number, number] {
  const length = words.length * LABEL_CHAR_PX;
  return anchor === "end" ? [x - length, x] : [x, x + length];
}

/**
 * The end anchor's label on an axis `width` px wide: `full`; where that would run over the Today
 * (or Start) label, just "Ends"; and where even that would, nothing (Today wins; the end line
 * stays, its hover title and the sentence under the bar name the milestone). `now` is where
 * the Today label sits (none when it is off the axis).
 */
export function endLabelWords(full: string, end: { at: number; anchor: "start" | "end" }, now: { at: number; anchor: "start" | "end"; words: string } | undefined, width: number): string {
  if (now === undefined || width === 0) {
    return full;
  }
  const [nowFrom, nowTo] = spanOf(now.words, now.at * width + (now.anchor === "end" ? -4 : 4), now.anchor);
  const clears = (words: string) => {
    const [from, to] = spanOf(words, end.at * width + (end.anchor === "end" ? -4 : 4), end.anchor);
    return !(from < nowTo + LABEL_GAP_PX && nowFrom < to + LABEL_GAP_PX);
  };
  return clears(full) ? full : clears("Ends") ? "Ends" : "";
}

const RANGE_WORDS: Record<Range, string> = { fit: "Fit", month: "Month", quarter: "Quarter" };

/** The timeline's own settings, in the address: the range. */
export function timelineSettingsFrom(params: URLSearchParams): { range: Range } {
  return { range: RANGES.find((each) => each === params.get("range")) ?? "fit" };
}

function percent(at: number): string {
  return `${(at * 100).toFixed(3)}%`;
}

function Defs() {
  return (
    <defs>
      <pattern id="timeline-hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
        <line x1="0" y1="0" x2="0" y2="6" className="timeline-hatch" />
      </pattern>
    </defs>
  );
}

/** What every row's track draws behind it: the ticks, what lies after the end, today (or the start), the end. */
function Backdrop({ axis, height }: { axis: Axis; height: number }) {
  const end = axis.anchor === undefined ? undefined : positionOf(axis, axis.anchor.day);
  return (
    <>
      {end === undefined ? null : <rect className="timeline-after" x={percent(end)} y={0} width={percent(Math.max(0, 1 - end))} height={height} />}
      {axis.ticks.map((tick) => (
        <line key={tick.day} className="timeline-grid" x1={percent(tick.at)} x2={percent(tick.at)} y1={0} y2={height} />
      ))}
      <line className="timeline-today" x1={percent(axis.now)} x2={percent(axis.now)} y1={0} y2={height} />
      {end === undefined ? null : <line className="timeline-end" x1={percent(end)} x2={percent(end)} y1={0} y2={height} />}
    </>
  );
}

/** The width (px) an element has, kept as the window resizes; 0 until it is measured. */
function useWidth(target: React.RefObject<Element | null>): number {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const element = target.current;
    if (element === null) {
      return;
    }
    // Observed, not read on a window resize: the inspector opening or closing narrows the axis too.
    const observer = new ResizeObserver(() => { setWidth(element.getBoundingClientRect().width); });
    observer.observe(element);
    return () => { observer.disconnect(); };
  }, [target]);
  return width;
}

/** C13: the axis: its ticks, today (the start, on a route), and the end anchor, or the words that there is none. */
function AxisHead({ axis, endTitle }: { axis: Axis; endTitle: (node: string) => string }) {
  const svg = useRef<SVGSVGElement>(null);
  const width = useWidth(svg);
  const end = axis.anchor;
  const anchorAt = end === undefined ? undefined : positionOf(axis, end.day);
  const textAnchor = (at: number) => (at > LABEL_FLIP_AT ? "end" : "start");
  const dx = (at: number) => (at > LABEL_FLIP_AT ? -4 : 4);
  const [first, second] = axis.ticks;
  const apart = width > 0 && first !== undefined && second !== undefined ? width * (second.at - first.at) : 0;
  // Names no closer than they fit, and none running off the right edge or across the end line.
  const every = apart > 0 ? Math.ceil(TICK_LABEL_PX / apart) : Math.ceil(axis.ticks.length / TICK_LABEL_COUNT_MAX);
  const clear = (tick: { at: number }) => width === 0 || (tick.at * width < width - TICK_LABEL_EDGE_PX && (anchorAt === undefined || tick.at >= anchorAt || (anchorAt - tick.at) * width >= TICK_LABEL_EDGE_PX));
  const named = (tick: { at: number }, at: number) => at % every === 0 && clear(tick);
  const nowWords = axis.mode === "calendar" ? "Today" : "Start";
  const nowShown = axis.now >= 0 && axis.now <= 1;
  const endWords = end === undefined ? "" : `Ends: ${endTitle(end.node)}${axis.mode === "relative" ? ` · ${dayWords(end.day)}` : ""}`;
  return (
    <svg ref={svg} className="timeline-axis-svg" width="100%" height={AXIS_PX} aria-hidden="true" data-testid="timeline-axis" data-unit={axis.unit}>
      <Backdrop axis={axis} height={AXIS_PX} />
      {axis.ticks.map((tick, at) => (
        // Every few ticks are named, so the names do not run into each other; the rest are lines.
        <text key={tick.day} className="timeline-tick" x={percent(tick.at)} y={15} dx={4} data-testid="timeline-tick" data-day={tick.day}>
          {named(tick, at) ? tick.label : ""}
        </text>
      ))}
      {nowShown ? (
        <text className="timeline-today-label" x={percent(axis.now)} y={32} dx={dx(axis.now)} textAnchor={textAnchor(axis.now)} data-testid="timeline-now">
          {nowWords}
        </text>
      ) : null}
      {end === undefined || anchorAt === undefined || width === 0 ? null : (
        // A hover target on the end line itself, so the words are there when the label gave way to Today.
        <rect x={percent(anchorAt - 4 / width)} y={0} width={8} height={AXIS_PX} fill="transparent">
          <title>{endWords}</title>
        </rect>
      )}
      {end === undefined || anchorAt === undefined ? null : (
        <text className="timeline-end-label" x={percent(anchorAt)} y={32} dx={dx(anchorAt)} textAnchor={textAnchor(anchorAt)}>
          {endLabelWords(
            endWords,
            { at: anchorAt, anchor: textAnchor(anchorAt) },
            nowShown ? { at: axis.now, anchor: textAnchor(axis.now), words: nowWords } : undefined,
            width,
          )}
        </text>
      )}
    </svg>
  );
}

/** One row's mark on its track: a diamond, a tick, a bar, or a container's span filled by how far it is. */
function Mark({ row, axis, y }: { row: ChartRow; axis: Axis; y: number }) {
  const from = positionOf(axis, row.from);
  const to = positionOf(axis, row.to);
  const classes = `timeline-mark timeline-shape-${row.shape}${row.late > 0 || row.shortfallDays !== undefined ? " timeline-bad" : ""}`;
  switch (row.shape) {
    case "milestone":
    case "decision": {
      const size = row.end ? DIAMOND_PX + 2 : DIAMOND_PX;
      return (
        <svg x={percent(to)} y={y} overflow="visible">
          <polygon className={classes} points={`0,${String(-size)} ${String(size)},0 0,${String(size)} ${String(-size)},0`} />
        </svg>
      );
    }
    case "tick":
      return <line className={classes} x1={percent(to)} x2={percent(to)} y1={y - BAR_PX - 2} y2={y + BAR_PX + 2} />;
    default: {
      const width = Math.max(to - from, BAR_WIDTH_MIN);
      const done = row.progress === undefined || row.progress.total === 0 ? 0 : row.progress.done / row.progress.total;
      return (
        <>
          <rect className={classes} x={percent(from)} y={y - BAR_PX} width={percent(width)} height={BAR_PX * 2} />
          {row.shape === "span" && done > 0 ? <rect className="timeline-fill" x={percent(from)} y={y - BAR_PX} width={percent(width * done)} height={BAR_PX * 2} /> : null}
        </>
      );
    }
  }
}

/** The lines from the selected row to what it needs and unblocks, drawn in the trace's styles. */
function Lines({ lines, axis, rows }: { lines: Dependency[]; axis: Axis; rows: ChartRow[] }) {
  return (
    <>
      {lines.map((line) => {
        const from = rows.findIndex((row) => row.key === line.from);
        const to = rows.findIndex((row) => row.key === line.to);
        const upstream = rows[from];
        const downstream = rows[to];
        if (upstream === undefined || downstream === undefined) {
          return null;
        }
        const x1 = positionOf(axis, upstream.to);
        const x2 = positionOf(axis, downstream.from);
        const mid = (x1 + x2) / 2;
        const y1 = from * ROW_PX + ROW_PX / 2;
        const y2 = to * ROW_PX + ROW_PX / 2;
        return (
          <g key={`${line.kind}:${line.from}:${line.to}`} className={`timeline-line timeline-line-${line.kind}`} data-testid="timeline-line" data-kind={line.kind} data-from={line.from} data-to={line.to}>
            <line x1={percent(x1)} x2={percent(mid)} y1={y1} y2={y1} />
            <line x1={percent(mid)} x2={percent(mid)} y1={y1} y2={y2} />
            <line x1={percent(mid)} x2={percent(x2)} y1={y2} y2={y2} />
            <svg x={percent(x2)} y={y2} overflow="visible">
              <circle r={3} />
            </svg>
          </g>
        );
      })}
    </>
  );
}

function Label({ axis, row, selected, onFold, title, mark }: { axis: Axis; row: ChartRow; selected: boolean; onFold: (key: string) => void; title: (key: string) => ReactNode; mark: DiffMark | undefined }) {
  return (
    <li
      className="timeline-label"
      data-testid="timeline-entry"
      data-node={row.key}
      data-day={row.to}
      data-date={axis.mode === "calendar" ? dateOf(row.to) : undefined}
      data-origin={row.origin ?? ""}
      data-overdue={row.late > 0}
      data-shortfall={row.shortfallDays ?? ""}
      data-end={row.end}
      data-state={row.state}
      data-shape={row.shape}
      data-faded={row.faded}
      data-selected={selected}
      data-container={row.shape === "span"}
      data-diff={mark?.kind}
      data-choice={mark?.choice === true ? true : undefined}
      style={{ height: ROW_PX }}
    >
      <div className="timeline-title" style={{ "--depth": row.depth } as CSSProperties}>
        <TreeGuides depth={row.depth} guides={row.guides} open={row.open} />
        <TreeFold container={row.foldable} open={row.open} label={row.title} testId="timeline-fold" onFold={() => { onFold(row.key); }} />
        <div className="timeline-text" title={row.title}>
          {title(row.key)}
          <span className={row.late > 0 || row.shortfallDays !== undefined ? "small timeline-words timeline-late" : "muted small timeline-words"} data-testid="timeline-words" data-marked={mark === undefined ? undefined : true} title={row.words}>
            {mark === undefined ? null : (
              <span className="layer-word" data-testid="row-diff" data-diff={mark.kind}>
                {DIFF_LABELS[mark.kind]}
                {mark.choice === true ? <span className="layer-choice"> · needs a choice</span> : null}
                {" · "}
              </span>
            )}
            {mark === undefined ? row.words : <span className="timeline-words-rest">{row.words}</span>}
          </span>
        </div>
      </div>
    </li>
  );
}

/** C13: pan and zoom the time axis from the trackpad: sideways scroll pans, a pinch (a ctrl-wheel) zooms. Vertical scroll is left to the page. */
function useGestures(target: React.RefObject<HTMLElement | null>, window: Window, onWindow: (window: Window) => void) {
  const latest = useRef({ window, onWindow });
  latest.current = { window, onWindow };
  useEffect(() => {
    const element = target.current;
    if (element === null) {
      return;
    }
    const wheel = (event: WheelEvent) => {
      const { window: shown, onWindow: set } = latest.current;
      const track = element.querySelector(".timeline-tracks")?.getBoundingClientRect();
      if (track === undefined || track.width === 0) {
        return;
      }
      const span = shown.last - shown.first;
      if (event.ctrlKey) {
        event.preventDefault();
        const next = Math.min(SPAN_DAYS_MAX, Math.max(SPAN_DAYS_MIN, span * Math.exp(event.deltaY * 0.01)));
        const at = Math.min(1, Math.max(0, (event.clientX - track.left) / track.width));
        const first = shown.first + (span - next) * at;
        set({ first, last: first + next });
      } else if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
        event.preventDefault();
        const days = (event.deltaX / track.width) * span;
        set({ first: shown.first + days, last: shown.last + days });
      }
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return () => {
      element.removeEventListener("wheel", wheel);
    };
  }, [target]);
}

/** A selected row far down the chart is brought into view, so its float and lines can be seen. */
function useReveal(chart: React.RefObject<HTMLElement | null>, selected: string | undefined) {
  useEffect(() => {
    if (selected !== undefined) {
      [...(chart.current?.querySelectorAll(".timeline-label") ?? [])].find((label) => label instanceof HTMLElement && label.dataset["node"] === selected)?.scrollIntoView({ block: "nearest" });
    }
  }, [chart, selected]);
}

/** The tracks: one drawing beside the labels, so every row's date falls where the axis says. */
function Tracks({ chart, axis, selected, activate, marks }: { chart: Chart; axis: Axis; selected: string | undefined; activate: ((key: string) => void) | undefined; marks: Record<string, DiffMark> | undefined }) {
  const height = chart.rows.length * ROW_PX;
  return (
    <div className="timeline-canvas">
      <svg className="timeline-tracks" width="100%" height={height} aria-hidden="true">
        <Defs />
        {chart.rows.map((row, at) => (row.shape === "span" ? <rect key={row.key} className="timeline-band" x={0} y={at * ROW_PX} width="100%" height={ROW_PX} /> : null))}
        {chart.rows.map((row, at) => (marks?.[row.key] === undefined ? null : <rect key={`diff-${row.key}`} className="timeline-diff" data-diff={marks[row.key]?.kind} x={0} y={at * ROW_PX} width="100%" height={ROW_PX} />))}
        <Backdrop axis={axis} height={height} />
        {chart.rows.map((row, at) => {
          const y = at * ROW_PX + ROW_PX / 2;
          const floatFrom = row.float === undefined ? undefined : positionOf(axis, row.float.from);
          const floatTo = row.float === undefined ? undefined : positionOf(axis, row.float.to);
          return (
            <g key={row.key} className="timeline-row" data-state={row.state} data-faded={row.faded} data-selected={row.key === selected} data-testid="timeline-track" data-node={row.key}>
              <rect className="timeline-hit" x={0} y={at * ROW_PX} width="100%" height={ROW_PX} data-activatable={activate !== undefined} onClick={activate === undefined ? undefined : () => { activate(row.key); }} />
              {row.late > 0 ? <rect className="timeline-overdue" x={percent(positionOf(axis, row.to))} y={y - BAR_PX} width={percent(Math.max(0, axis.now - positionOf(axis, row.to)))} height={BAR_PX * 2} /> : null}
              {floatFrom === undefined || floatTo === undefined ? null : (
                <g className="timeline-float" data-testid="timeline-float" data-from={row.float?.from} data-to={row.float?.to}>
                  <line x1={percent(floatFrom)} x2={percent(floatTo)} y1={y} y2={y} />
                  <line x1={percent(floatFrom)} x2={percent(floatFrom)} y1={y - 5} y2={y + 5} />
                  <line x1={percent(floatTo)} x2={percent(floatTo)} y1={y - 5} y2={y + 5} />
                </g>
              )}
              <Mark row={row} axis={axis} y={y} />
            </g>
          );
        })}
        <Lines lines={chart.dependencies} axis={axis} rows={chart.rows} />
      </svg>
    </div>
  );
}

/** The way back from a pan or a pinch: the range's window again, or Fit, the whole span, which a route has with no presets. */
function ResetPan({ range, panned, onReset }: { range: Range | undefined; panned: boolean; onReset: () => void }) {
  if (!panned) {
    return null;
  }
  return (
    <div className="row timeline-controls">
      <Button ghost data-testid="timeline-reset" onClick={onReset}>
        {range === undefined ? "Fit" : `Back to ${RANGE_WORDS[range].toLowerCase()}`}
      </Button>
    </div>
  );
}

/** A journey's range presets, one switch at the end of the view controls' row (a new range starts the pan afresh, so choosing is only navigation). */
function RangeSwitch({ range }: { range: Range }) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  return (
    <Segmented
      label="Range"
      value={range}
      options={RANGES.map((each) => ({ value: each, label: RANGE_WORDS[each] }))}
      onChange={(value) => { void navigate(`${pathname}${withParam(search, "range", value === "fit" ? undefined : value)}`); }}
      data-testid="timeline-range"
    />
  );
}

/** C13: the axis the rows lie on: the range's window, or the one a pan or a pinch moved to, which holds until the range or the step is chosen again. */
function useAxis(spec: AxisSpec, days: readonly number[], key: string, preset: Window | undefined) {
  const [dragged, setDragged] = useState<Window | undefined>(undefined);
  // A new range or step starts afresh, and coming back to an old one does not bring its pan back.
  useEffect(() => { setDragged(undefined); }, [key]);
  const base = useMemo(() => axisOf(spec, days, preset), [spec, days, preset?.first, preset?.last]);
  const held = dragged;
  const axis = useMemo(() => (held === undefined ? base : axisOf(spec, [], { first: Math.round(held.first), last: Math.round(held.last) })), [base, held, spec]);
  return { axis, panned: held !== undefined, window: held ?? { first: base.start, last: base.end }, move: (window: Window) => { setDragged(window); }, reset: () => { setDragged(undefined); } };
}

export interface TimelineChartProps {
  chart: Chart;
  /** The axis the chart lies on: a journey's calendar, or a route's days from its start. */
  spec: AxisSpec;
  /** A new key starts a pan and a pinch afresh: the step and range chosen. */
  viewKey: string;
  /** A journey's range presets; none for a chart that opens on its whole span. */
  range: Range | undefined;
  onFold: (key: string) => void;
  selected: string | undefined;
  /** A row's title, and a node's in the folded list of those with no day. */
  title: (key: string) => ReactNode;
  /** The words after "Ends:" for the end anchor. */
  endTitle: (node: string) => string;
  /** What a click on a row's track does; none when a row opens nothing. */
  activate: ((key: string) => void) | undefined;
  /** The line under the controls that says where the chart ends. */
  anchorNote: ReactNode;
  /** What follows the words when nothing is drawn, and the words when nothing at all has a day. */
  empty: { filtered: ReactNode; none: string };
  /** The folded list's summary: "No date yet", "No day yet". */
  undated: string;
  /** Whether the level drew anything with a day (else `empty.none` says why not). */
  anyScheduled: boolean;
  /** C14: under the proposal layer, the mark on each row it changes, and the titles of what it removes (which have no row after). */
  layer?: { marks: Record<string, DiffMark>; removed: { key: string; title: string }[] } | undefined;
}

/** C13: a timeline: the rows of `chart` on one axis, with the controls, the end anchor's note and the folded list of what has no day. */
export function TimelineChart({ chart, spec, viewKey, range, onFold, selected, title, endTitle, activate, anchorNote, empty, undated, anyScheduled, layer }: TimelineChartProps) {
  const preset = range === undefined ? undefined : rangeWindow(range, dateOf(spec.now));
  const { axis, panned, window, move, reset } = useAxis(spec, chart.days, viewKey, preset);
  const chartRef = useRef<HTMLDivElement>(null);
  useGestures(chartRef, window, move);
  useReveal(chartRef, selected);
  return (
    <section
      className="stack timeline"
      aria-label="Timeline"
      data-testid="timeline"
      data-end={axis.anchor?.node ?? ""}
      data-end-date={axis.anchor === undefined || axis.mode !== "calendar" ? "" : dateOf(axis.anchor.day)}
      data-end-day={axis.anchor === undefined ? "" : axis.anchor.day}
    >
      <ResetPan range={range} panned={panned} onReset={reset} />
      <span className="muted small" data-testid="timeline-anchor">
        {anchorNote}
      </span>
      {chart.rows.length === 0 ? (
        <EmptyPanel testId="timeline-empty">{anyScheduled ? empty.filtered : empty.none}</EmptyPanel>
      ) : (
        <div className="timeline-chart" ref={chartRef}>
          <div className="timeline-axis">
            <span />
            <AxisHead axis={axis} endTitle={endTitle} />
          </div>
          <div className="timeline-body">
            <ol className="plain-list timeline-labels">
              {chart.rows.map((row) => (
                <Label key={row.key} axis={axis} row={row} selected={row.key === selected} onFold={onFold} title={title} mark={layer?.marks[row.key]} />
              ))}
            </ol>
            <Tracks chart={chart} axis={axis} selected={selected} activate={activate} marks={layer?.marks} />
          </div>
        </div>
      )}
      {layer === undefined || layer.removed.length === 0 ? null : (
        <details className="timeline-removed" data-testid="timeline-removed" open>
          <summary>Removed by this proposal ({layer.removed.length})</summary>
          <ul className="timeline-undated-list">
            {layer.removed.map((each) => (
              <li key={each.key} data-node={each.key} data-diff="removed">
                <span className="layer-word">{DIFF_LABELS.removed}</span> · {each.title}
              </li>
            ))}
          </ul>
        </details>
      )}
      {chart.undated.length === 0 ? null : (
        <details className="timeline-undated" data-testid="timeline-undated" open={selected !== undefined && chart.undated.includes(selected) ? true : undefined}>
          <summary>
            {undated} ({chart.undated.length})
          </summary>
          <ul className="timeline-undated-list">
            {chart.undated.map((node) => (
              <li key={node} data-node={node}>
                {title(node)}
                {chart.undatedWords?.get(node) === undefined ? null : <span className="muted small"> {chart.undatedWords.get(node)}</span>}
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}

/** C13, C2: the timeline view: the toolbar, then the dated nodes of the journey's level, folded by the ladder and the viewer's flips (which hold until another depth), lit by Frontier and dimmed below the depth by Fade. */
export function ProjectedTimeline({ ready, state, picks, selected }: { ready: Ready; state: ViewState; picks: number; selected: string | undefined }) {
  const { search } = useLocation();
  const addresses = useNodeAddresses();
  const { range } = useMemo(() => timelineSettingsFrom(new URLSearchParams(search)), [search]);
  const nodes = ready.journey.graph.nodes ?? [];
  const layered = useLayer();
  const { step, current, level, error: levelError, dimmed: faded, badges } = useViewLevel({ screen: "timeline", state, nodes, selected, ready, read: (request, at) => useLevel(ready, request, at) });
  const timelineRead = useProjected(ready, { projection: "timeline" });
  const timeline = valueOf(timelineRead);
  const navigate = useNavigate();
  const { open, fold: onFold } = useFolds(nodes, current, state.depth, step, picks, selected, (key) => void navigate(addresses.open(key)));
  const error = levelError ?? errorWords(timelineRead);
  const today = ready.derived.today;
  const spec = useMemo(() => (timeline === undefined ? undefined : calendarSpec(timeline, today)), [timeline, today]);
  const layer = useMemo(() => {
    if (layered?.mode !== "reviewing" || layered.model === undefined) {
      return undefined;
    }
    const was = layered.before?.journey.graph.nodes ?? [];
    return { marks: layered.model.marks, removed: [...layered.removed].map((key) => ({ key, title: was.find((node) => node.key === key)?.title ?? key })) };
  }, [layered]);
  const chart = useMemo(() => (timeline === undefined || level === undefined ? undefined : chartOf(ready, timeline, { drawn: level.nodes, open, selected, faded })), [ready, timeline, level, open, selected, faded]);
  return (
    <>
      <ViewBar subject={journeySubject(ready)} screen="timeline" badges={badges} end={<RangeSwitch range={range} />} />
      {error !== undefined ? (
        <p className="callout callout-bad"><Failed what="The timeline" error={error} /></p>
      ) : timeline === undefined || level === undefined || spec === undefined || chart === undefined ? (
        <p className="muted small">Placing the dates...</p>
      ) : (
        <TimelineChart
          chart={chart}
          spec={spec}
          viewKey={`${range}:${step}`}
          range={range}
          onFold={onFold}
          selected={selected}
          title={(key) => <NodeLink view={ready} node={key} />}
          endTitle={(node) => titleOf(ready, node)}
          activate={(key) => void navigate(addresses.open(key))}
          anchorNote={
            spec.anchor === undefined ? (
              "No final milestone has a date yet, so the timeline ends after its latest date."
            ) : (
              <>
                Ends at <NodeLink view={ready} node={spec.anchor.node} /> on <strong>{dateWords(dateOf(spec.anchor.day), today)}</strong>, the final milestone.
              </>
            )
          }
          empty={{ filtered: <NothingMatches />, none: "Nothing in scope has a date yet: no milestone date, pin, or due date." }}
          undated="No date yet"
          anyScheduled={timeline.entries.length > 0}
          layer={layer}
        />
      )}
    </>
  );
}
