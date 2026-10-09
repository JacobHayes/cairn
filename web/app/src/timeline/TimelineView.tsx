// C13: the timeline. One row per date (the projection's order, earliest first): the node and
// its date, told apart as actual, pin, or derived due (F7), with overdue and shortfall marked,
// beside a track in plain SVG on one shared time axis with today and the end anchor (the
// `final` milestone) drawn across every row; what lies after the end is shaded. A row's "Why"
// shows the node's dates with their chains, as node detail says them (F7), and its title opens
// the node's detail (5.1) beside the timeline. Undated milestones are listed after.
import { useMemo, useState } from "react";

import { DatesSection } from "../detail/DatesSection.tsx";
import { nodeDetail, nodeOf, titleOf, type Ready } from "../detail/model.ts";
import { NodeLink } from "../detail/parts.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { positionOf, timelineAxis, timelineKeeps, timelineRows, type Axis, type Timeline, type TimelineNarrowing, type TimelineRow } from "./model.ts";
import "./timeline.css";

/** The height of one row's track, and of the axis above the rows (px). */
const ROW_PX = 36;
const AXIS_PX = 44;

/** A marker's half width (px): a diamond for a milestone, a circle for anything else. */
const MARKER_PX = 7;

/** Past this fraction of the width, a label on the axis reads leftward, so it stays on it. */
const LABEL_FLIP_AT = 0.75;

function percent(at: number): string {
  return `${(at * 100).toFixed(3)}%`;
}

const ORIGIN_WORDS = { actual: "actual", pin: "pin", due: "derived due" } as const;

/** What every row's track draws behind its marker: the ticks, what lies after the end, today, the end. */
function Backdrop({ axis }: { axis: Axis }) {
  const end = axis.anchor === undefined ? undefined : positionOf(axis, axis.anchor.date);
  return (
    <>
      {end === undefined ? null : <rect className="timeline-after" x={percent(end)} y={0} width={percent(1 - end)} height="100%" />}
      {axis.ticks.map((tick) => (
        <line key={tick.date} className="timeline-grid" x1={percent(tick.at)} x2={percent(tick.at)} y1={0} y2="100%" />
      ))}
      <line className="timeline-today" x1={percent(axis.today)} x2={percent(axis.today)} y1={0} y2="100%" />
      {end === undefined ? null : <line className="timeline-end" x1={percent(end)} x2={percent(end)} y1={0} y2="100%" />}
    </>
  );
}

/** C13: the axis: its ticks, then today and the end anchor beneath them. */
function AxisHead({ ready, axis }: { ready: Ready; axis: Axis }) {
  const end = axis.anchor;
  const anchorAt = end === undefined ? undefined : positionOf(axis, end.date);
  const textAnchor = (at: number) => (at > LABEL_FLIP_AT ? "end" : "start");
  const dx = (at: number) => (at > LABEL_FLIP_AT ? -3 : 3);
  return (
    <svg className="timeline-axis" width="100%" height={AXIS_PX} aria-hidden="true" data-testid="timeline-axis" data-unit={axis.unit}>
      <Backdrop axis={axis} />
      {axis.ticks.map((tick) => (
        <text key={tick.date} className="timeline-tick" x={percent(tick.at)} y={14} dx={3} data-testid="timeline-tick" data-date={tick.date}>
          {tick.label}
        </text>
      ))}
      <text className="timeline-today-label" x={percent(axis.today)} y={34} dx={dx(axis.today)} textAnchor={textAnchor(axis.today)}>
        Today
      </text>
      {end === undefined || anchorAt === undefined ? null : (
        <text className="timeline-end-label" x={percent(anchorAt)} y={34} dx={dx(anchorAt)} textAnchor={textAnchor(anchorAt)}>
          End: {titleOf(ready, end.node)}
        </text>
      )}
    </svg>
  );
}

function Marker({ row }: { row: TimelineRow }) {
  const classes = ["timeline-marker", `timeline-marker-${row.origin}`, row.overdue || row.shortfallDays !== undefined ? "timeline-marker-bad" : ""];
  const size = row.end ? MARKER_PX + 2 : MARKER_PX;
  return (
    <svg x={percent(row.at)} y={ROW_PX / 2} overflow="visible">
      {row.kind === "milestone" ? (
        <polygon className={classes.join(" ")} points={`0,${String(-size)} ${String(size)},0 0,${String(size)} ${String(-size)},0`} />
      ) : (
        <circle className={classes.join(" ")} r={size - 1} />
      )}
    </svg>
  );
}

function Marks({ row }: { row: TimelineRow }) {
  return (
    <>
      {row.end ? <Badge data-testid="timeline-end">end</Badge> : null}
      {row.overdue ? <Badge tone="bad" data-testid="timeline-overdue">overdue</Badge> : null}
      {row.shortfallDays === undefined ? null : (
        <Badge tone="bad" data-testid="timeline-shortfall" data-status={String(row.shortfallDays)}>
          {row.shortfallDays} days short
        </Badge>
      )}
    </>
  );
}

function Row({ ready, axis, row, selected }: { ready: Ready; axis: Axis; row: TimelineRow; selected: boolean }) {
  const [why, setWhy] = useState(false);
  const detail = why ? nodeDetail(ready, row.node) : undefined;
  return (
    <li
      className="timeline-row"
      data-testid="timeline-entry"
      data-node={row.node}
      data-date={row.date}
      data-origin={row.origin}
      data-overdue={row.overdue}
      data-shortfall={row.shortfallDays ?? ""}
      data-end={row.end}
      data-selected={selected}
    >
      <div className="timeline-label">
        <NodeLink view={ready} node={row.node} />
        <span className="muted small">
          {row.kind}, {row.date}
        </span>
        <Badge data-testid="timeline-origin">{ORIGIN_WORDS[row.origin]}</Badge>
        <Marks row={row} />
        <Button className="timeline-why no-print" aria-expanded={why} onClick={() => { setWhy(!why); }}>
          {why ? "Hide why" : "Why"}
        </Button>
      </div>
      <svg className="timeline-track" width="100%" height={ROW_PX} aria-hidden="true">
        <Backdrop axis={axis} />
        <Marker row={row} />
      </svg>
      {detail === undefined ? null : (
        <div className="timeline-explain" data-testid="timeline-why">
          <DatesSection view={ready} detail={detail} />
        </div>
      )}
    </li>
  );
}

function Legend() {
  const swatch = (className: string, milestone: boolean) => (
    <svg width={18} height={18} aria-hidden="true">
      <svg x={9} y={9} overflow="visible">
        {milestone ? <polygon className={`timeline-marker ${className}`} points="0,-7 7,0 0,7 -7,0" /> : <circle className={`timeline-marker ${className}`} r={6} />}
      </svg>
    </svg>
  );
  return (
    <div className="row muted small timeline-legend" aria-label="Legend">
      <span>{swatch("timeline-marker-actual", true)} actual (reached)</span>
      <span>{swatch("timeline-marker-pin", false)} pinned</span>
      <span>{swatch("timeline-marker-due", false)} derived due date</span>
      <span>{swatch("timeline-marker-due timeline-marker-bad", false)} overdue or short</span>
      <span>Diamonds are milestones.</span>
    </div>
  );
}

/** C13: the timeline of a projected journey, narrowed by the toolbar; the axis stays that of every date. */
export function TimelineChart({ ready, timeline, narrowing, selected }: { ready: Ready; timeline: Timeline; narrowing: TimelineNarrowing; selected: string | undefined }) {
  const axis = useMemo(() => timelineAxis(timeline, ready.derived.today), [timeline, ready.derived.today]);
  const rows = useMemo(
    () => timelineRows(ready, timeline, axis).filter((row) => timelineKeeps(narrowing, row.kind, row.title)),
    [ready, timeline, axis, narrowing],
  );
  const undated = (timeline.undated ?? []).filter((key) => {
    const kind = nodeOf(ready, key)?.kind;
    return kind !== undefined && timelineKeeps(narrowing, kind, titleOf(ready, key));
  });
  return (
    <section className="stack" aria-label="Timeline" data-testid="timeline" data-end={axis.anchor?.node ?? ""} data-end-date={axis.anchor?.date ?? ""}>
      <div className="row">
        <span data-testid="timeline-anchor">
          {axis.anchor === undefined ? (
            <span className="muted small">No final milestone with a date: the timeline ends after its latest date.</span>
          ) : (
            <>
              Ends at <NodeLink view={ready} node={axis.anchor.node} /> on <strong>{axis.anchor.date}</strong>, the final milestone.
            </>
          )}
        </span>
      </div>
      <Legend />
      {rows.length === 0 ? (
        <p className="muted small" data-testid="timeline-empty">
          {timeline.entries.length > 0 ? "Nothing dated matches the toolbar." : "Nothing in scope has a date yet: no milestone date, pin, or due date."}
        </p>
      ) : (
        <ol className="timeline-rows">
          <li className="timeline-row timeline-row-axis">
            <span />
            <AxisHead ready={ready} axis={axis} />
          </li>
          {rows.map((row) => (
            <Row key={row.node} ready={ready} axis={axis} row={row} selected={row.node === selected} />
          ))}
        </ol>
      )}
      {undated.length === 0 ? null : (
        <span data-testid="timeline-undated">
          No date yet:{" "}
          {undated.map((key, index) => (
            <span key={key} data-node={key}>
              {index === 0 ? "" : ", "}
              <NodeLink view={ready} node={key} />
            </span>
          ))}
        </span>
      )}
    </section>
  );
}
