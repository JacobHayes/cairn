// C13: the timeline. Rows follow the containment tree and fold by the detail ladder; each
// has its date in plain words beside a track on one shared time axis with today and the end
// anchor (the `final` milestone) drawn across it. Selecting a row shows its float and the lines
// to what it needs and unblocks (a shortfall's chain in error ink) and fades the rest; the
// toolbar's chips fade what they exclude. Two-finger horizontal scroll pans time, a pinch
// zooms it, vertical scroll moves the rows (the workspace's own). Nodes with no date gather in
// a folded list. A row's title opens the node's detail (5.1) beside the timeline, which
// explains its dates (F7).
import { useEffect, useMemo, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import { nodeOf, titleOf, type Ready } from "../detail/model.ts";
import { NodeLink, nodePath, screenPath } from "../detail/parts.tsx";
import { withParam } from "../journeys/address.ts";
import { Button, Segmented } from "../ui/kit.tsx";
import { dateWords, dayOf, positionOf, rangeWindow, RANGES, timelineAxis, type Axis, type Range, type Timeline, type Window } from "./model.ts";
import { chartOf, DETAILS, type Chart, type ChartRow, type Dependency, type Detail, type Narrowing } from "./rows.ts";
import "./timeline.css";

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

/** The least room (px) a tick's name takes along the axis, and what a name keeps clear of the right edge. */
const TICK_LABEL_PX = 68;
const TICK_LABEL_EDGE_PX = 48;

/** The fewest and most days a pinch leaves in view. */
const SPAN_DAYS_MIN = 7;
const SPAN_DAYS_MAX = 3650;

/** Past this fraction of the width, a label on the axis reads leftward, so it stays on it. */
const LABEL_FLIP_AT = 0.75;

const DETAIL_WORDS: Record<Detail, string> = { stages: "Stages", decisions: "Decisions", work: "Work", all: "All" };
const RANGE_WORDS: Record<Range, string> = { fit: "Fit", month: "Month", quarter: "Quarter" };

/** The timeline's own settings, in the address: the detail step and the range. */
export function timelineFrom(params: URLSearchParams): { detail: Detail; range: Range } {
  return {
    detail: DETAILS.find((each) => each === params.get("detail")) ?? "stages",
    range: RANGES.find((each) => each === params.get("range")) ?? "fit",
  };
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

/** What every row's track draws behind it: the ticks, what lies after the end, today, the end. */
function Backdrop({ axis, height }: { axis: Axis; height: number }) {
  const end = axis.anchor === undefined ? undefined : positionOf(axis, axis.anchor.date);
  return (
    <>
      {end === undefined ? null : <rect className="timeline-after" x={percent(end)} y={0} width={percent(Math.max(0, 1 - end))} height={height} />}
      {axis.ticks.map((tick) => (
        <line key={tick.date} className="timeline-grid" x1={percent(tick.at)} x2={percent(tick.at)} y1={0} y2={height} />
      ))}
      <line className="timeline-today" x1={percent(axis.today)} x2={percent(axis.today)} y1={0} y2={height} />
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

/** C13: the axis: its ticks, today, and the end anchor, or the words that there is none. */
function AxisHead({ ready, axis }: { ready: Ready; axis: Axis }) {
  const svg = useRef<SVGSVGElement>(null);
  const width = useWidth(svg);
  const end = axis.anchor;
  const anchorAt = end === undefined ? undefined : positionOf(axis, end.date);
  const textAnchor = (at: number) => (at > LABEL_FLIP_AT ? "end" : "start");
  const dx = (at: number) => (at > LABEL_FLIP_AT ? -4 : 4);
  const [first, second] = axis.ticks;
  const apart = width > 0 && first !== undefined && second !== undefined ? width * (second.at - first.at) : 0;
  // Names no closer than they fit, and none running off the right edge.
  const every = apart > 0 ? Math.ceil(TICK_LABEL_PX / apart) : Math.ceil(axis.ticks.length / TICK_LABEL_COUNT_MAX);
  const named = (tick: { at: number }, at: number) => at % every === 0 && (width === 0 || tick.at * width < width - TICK_LABEL_EDGE_PX);
  return (
    <svg ref={svg} className="timeline-axis-svg" width="100%" height={AXIS_PX} aria-hidden="true" data-testid="timeline-axis" data-unit={axis.unit}>
      <Backdrop axis={axis} height={AXIS_PX} />
      {axis.ticks.map((tick, at) => (
        // Every few ticks are named, so the names do not run into each other; the rest are lines.
        <text key={tick.date} className="timeline-tick" x={percent(tick.at)} y={15} dx={4} data-testid="timeline-tick" data-date={tick.date}>
          {named(tick, at) ? tick.label : ""}
        </text>
      ))}
      {axis.today < 0 || axis.today > 1 ? null : (
        <text className="timeline-today-label" x={percent(axis.today)} y={32} dx={dx(axis.today)} textAnchor={textAnchor(axis.today)}>
          Today
        </text>
      )}
      {end === undefined || anchorAt === undefined ? (
        <text className="timeline-end-label" x="100%" y={32} dx={-4} textAnchor="end" data-testid="timeline-no-end">
          No final milestone
        </text>
      ) : (
        <text className="timeline-end-label" x={percent(anchorAt)} y={32} dx={dx(anchorAt)} textAnchor={textAnchor(anchorAt)}>
          Ends: {titleOf(ready, end.node)}
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

function Label({ ready, row, selected, onFold }: { ready: Ready; row: ChartRow; selected: boolean; onFold: (key: string) => void }) {
  return (
    <li
      className="timeline-label"
      data-testid="timeline-entry"
      data-node={row.key}
      data-date={row.to}
      data-origin={row.origin ?? ""}
      data-overdue={row.late > 0}
      data-shortfall={row.shortfallDays ?? ""}
      data-end={row.end}
      data-state={row.state}
      data-shape={row.shape}
      data-faded={row.faded}
      data-selected={selected}
      style={{ height: ROW_PX }}
    >
      <div className="timeline-title" style={{ paddingInlineStart: `calc(${String(row.depth)} * var(--space-4))` }}>
        {row.foldable ? (
          <button type="button" className="timeline-fold" aria-expanded={row.open} aria-label={`${row.open ? "Fold" : "Open"} ${row.title}`} data-testid="timeline-fold" onClick={() => { onFold(row.key); }}>
            {row.open ? "▾" : "▸"}
          </button>
        ) : (
          <span className="timeline-fold" aria-hidden="true" />
        )}
        <div className="timeline-text">
          <NodeLink view={ready} node={row.key} />
          <span className={row.late > 0 || row.shortfallDays !== undefined ? "small timeline-words timeline-late" : "muted small timeline-words"} data-testid="timeline-words">
            {row.words}
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
function Tracks({ chart, axis, selected }: { chart: Chart; axis: Axis; selected: string | undefined }) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const height = chart.rows.length * ROW_PX;
  return (
    <svg className="timeline-tracks" width="100%" height={height} aria-hidden="true">
      <Defs />
      <Backdrop axis={axis} height={height} />
      {chart.rows.map((row, at) => {
        const y = at * ROW_PX + ROW_PX / 2;
        const floatFrom = row.float === undefined ? undefined : positionOf(axis, row.float.from);
        const floatTo = row.float === undefined ? undefined : positionOf(axis, row.float.to);
        return (
          <g key={row.key} className="timeline-row" data-state={row.state} data-faded={row.faded} data-selected={row.key === selected} data-testid="timeline-track" data-node={row.key}>
            <rect className="timeline-hit" x={0} y={at * ROW_PX} width="100%" height={ROW_PX} onClick={() => { void navigate({ pathname: nodePath(screenPath(pathname), row.key), search }); }} />
            {row.late > 0 ? <rect className="timeline-overdue" x={percent(positionOf(axis, row.to))} y={y - BAR_PX} width={percent(Math.max(0, axis.today - positionOf(axis, row.to)))} height={BAR_PX * 2} /> : null}
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
  );
}

/** The detail step and the range, and the way back from a pan or a pinch. */
function Controls({ detail, range, panned, onChoose, onReset }: { detail: Detail; range: Range; panned: boolean; onChoose: (name: string, value: string) => void; onReset: () => void }) {
  return (
    <div className="row timeline-controls">
      <Segmented label="Detail" value={detail} options={DETAILS.map((each) => ({ value: each, label: DETAIL_WORDS[each] }))} onChange={(value) => { onChoose("detail", value); }} data-testid="timeline-detail" />
      <Segmented label="Range" value={range} options={RANGES.map((each) => ({ value: each, label: RANGE_WORDS[each] }))} onChange={(value) => { onChoose("range", value); }} data-testid="timeline-range" />
      {panned ? (
        <Button ghost data-testid="timeline-reset" onClick={onReset}>
          Back to {RANGE_WORDS[range].toLowerCase()}
        </Button>
      ) : null}
    </div>
  );
}

/** C13: the axis the rows lie on: the range's window, or the one a pan or a pinch moved to, which holds until the range or the step is chosen again. */
function useAxis(timeline: Timeline, today: string, chart: Chart, key: string, range: Range) {
  const [dragged, setDragged] = useState<{ for: string; window: Window } | undefined>(undefined);
  const preset = rangeWindow(range, today);
  const base = useMemo(() => timelineAxis(timeline, today, chart.dates, preset), [timeline, today, chart.dates, preset?.first, preset?.last]);
  const held = dragged?.for === key ? dragged.window : undefined;
  const axis = useMemo(() => (held === undefined ? base : timelineAxis(timeline, today, [], { first: Math.round(held.first), last: Math.round(held.last) })), [base, held, timeline, today]);
  return { axis, panned: held !== undefined, window: held ?? { first: dayOf(base.start), last: dayOf(base.end) }, move: (window: Window) => { setDragged({ for: key, window }); }, reset: () => { setDragged(undefined); } };
}

/** C13: the timeline of a projected journey, narrowed by the toolbar. */
export function TimelineChart({ ready, timeline, narrow, selected, detail, range }: { ready: Ready; timeline: Timeline; narrow: Narrowing; selected: string | undefined; detail: Detail; range: Range }) {
  const { pathname, search } = useLocation();
  const navigate = useNavigate();
  const [flipped, setFlipped] = useState<ReadonlySet<string>>(new Set());
  const chart = useMemo(() => chartOf(ready, timeline, { detail, flipped, selected, narrow }), [ready, timeline, detail, flipped, selected, narrow]);
  const { axis, panned, window, move, reset } = useAxis(timeline, ready.derived.today, chart, `${range}:${detail}`, range);
  const chartRef = useRef<HTMLDivElement>(null);
  useGestures(chartRef, window, move);
  useReveal(chartRef, selected);
  const fold = (row: string) => {
    const next = new Set(flipped);
    if (!next.delete(row)) {
      next.add(row);
    }
    setFlipped(next);
  };
  const choose = (name: string, value: string) => {
    reset();
    if (name === "detail") {
      setFlipped(new Set());
    }
    void navigate(`${pathname}${withParam(search, name, value === (name === "detail" ? "stages" : "fit") ? undefined : value)}`);
  };
  return (
    <section className="stack timeline" aria-label="Timeline" data-testid="timeline" data-end={axis.anchor?.node ?? ""} data-end-date={axis.anchor?.date ?? ""}>
      <Controls detail={detail} range={range} panned={panned} onChoose={choose} onReset={reset} />
      <span className="muted small" data-testid="timeline-anchor">
        {axis.anchor === undefined ? (
          "No final milestone has a date yet, so the timeline ends after its latest date."
        ) : (
          <>
            Ends at <NodeLink view={ready} node={axis.anchor.node} /> on <strong>{dateWords(axis.anchor.date, ready.derived.today)}</strong>, the final milestone.
          </>
        )}
      </span>
      {chart.rows.length === 0 ? (
        <p className="muted" data-testid="timeline-empty">
          {timeline.entries.length > 0 ? "Nothing dated is shown at this level of detail." : "Nothing in scope has a date yet: no milestone date, pin, or due date."}
        </p>
      ) : (
        <div className="timeline-chart" ref={chartRef}>
          <div className="timeline-axis">
            <span />
            <AxisHead ready={ready} axis={axis} />
          </div>
          <div className="timeline-body">
            <ol className="timeline-labels">
              {chart.rows.map((row) => (
                <Label key={row.key} ready={ready} row={row} selected={row.key === selected} onFold={fold} />
              ))}
            </ol>
            <Tracks chart={chart} axis={axis} selected={selected} />
          </div>
        </div>
      )}
      {chart.undated.length === 0 ? null : (
        <details className="timeline-undated" data-testid="timeline-undated">
          <summary>No date yet ({chart.undated.length})</summary>
          <ul className="timeline-undated-list">
            {chart.undated.map((node) => (
              <li key={node} data-node={node} data-kind={nodeOf(ready, node)?.kind}>
                <NodeLink view={ready} node={node} />
              </li>
            ))}
          </ul>
        </details>
      )}
    </section>
  );
}
