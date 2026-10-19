// C13: a route's timeline: the route read as a journey that starts on day 0 and nothing is
// waiting on, so each node sits at the earliest day its dependencies, estimates, date rules and
// stage bounds allow. Rows are the nodes the level draws (Depth, Show conditional and the search
// every view of a route shares), under the containment tree and folded by the ladder; the chart
// is the journey timeline's (`TimelineChart`) on a relative axis, with a start line at day 0 and
// the final milestone as the end anchor. The bars are the before and after order; the relations
// themselves are on the Graph. A node with no earliest day gathers in a folded list, and a row
// whose date rule waits on a date decision says so. A row's title opens the node's form on a
// draft; a version's nodes open nothing.
import { useMemo } from "react";
import { Link, useNavigate } from "react-router";

import { treeOf } from "../authoring/graph.ts";
import { useFolds } from "../canvas/hooks.ts";
import { collapsedAt, drawnStep, stepOf } from "../canvas/ladder.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { useNodeAddresses } from "../journeys/address.ts";
import { badgesOf, depthFaded, levelRequestOf, outOfFocus, stepHides } from "../journeys/requests.ts";
import type { ViewState } from "../journeys/view.ts";
import { useRouteLevel, useRouteSchedule } from "../screens/route-views.ts";
import { nodesOf, type Shown } from "../screens/route-reads.ts";
import { NothingMatches, routeSubject, ViewBar } from "../screens/ViewControls.tsx";
import { routeSpec } from "./model.ts";
import { routeChartOf } from "./rows.ts";
import { TimelineChart } from "./TimelineView.tsx";
import { NODE_GONE } from "../ui/gone.ts";
import { Failed } from "../ui/Failed.tsx";

const NONE: ReadonlySet<string> = new Set();

export interface RouteTimelineProps {
  shown: Shown;
  state: ViewState;
  /** How many times a depth was picked: a pick drops the viewer's folds. */
  picks: number;
  selected: string | undefined;
  /** What opening a node does (a draft's opens its form, or is picked as the end of a requirement being drawn); none on a version, whose nodes open nothing. */
  onOpen: ((key: string) => void) | undefined;
}

export function RouteTimeline({ shown, state, picks, selected, onOpen }: RouteTimelineProps) {
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const nodes = nodesOf(shown);
  const tree = useMemo(() => treeOf(shown.graph), [shown.graph]);
  const depth = stepOf({ step: state.depth }, nodes);
  const step = drawnStep(depth, state.fade);
  const request = useMemo(() => levelRequestOf(state, step, []), [state, step]);
  const { level, drawn, error: levelError } = useRouteLevel(shown, request, selected);
  const scheduleRead = useRouteSchedule(shown);
  const schedule = valueOf(scheduleRead);
  const { open, fold: onFold } = useFolds(nodes, NONE, state.depth, step, picks, selected, (key) => void navigate(addresses.open(key)));
  const faded = useMemo(() => outOfFocus(undefined, drawn === undefined ? NONE : depthFaded(state, nodes, depth, collapsedAt(depth, nodes, NONE, [], []), drawn)), [state, nodes, depth, drawn]);
  const badges = useMemo(() => badgesOf("timeline", level?.hidden_by_class ?? {}, undefined, stepHides(nodes, depth, []), state.fade), [level, nodes, depth, state.fade]);
  const error = levelError ?? errorWords(scheduleRead);
  const spec = useMemo(() => (schedule === undefined ? undefined : routeSpec(schedule)), [schedule]);
  const chart = useMemo(() => (schedule === undefined || level === undefined ? undefined : routeChartOf(tree, schedule, { drawn: level.nodes, open, selected, faded })), [tree, schedule, level, open, selected, faded]);
  const titleOf = (key: string) => tree.byKey.get(key)?.title ?? NODE_GONE;
  const end = schedule?.end;
  return (
    <>
      <ViewBar subject={routeSubject(nodes)} screen="timeline" badges={badges} />
      {error !== undefined ? (
        <p className="callout callout-bad"><Failed what="The timeline" error={error} /></p>
      ) : schedule === undefined || level === undefined || spec === undefined || chart === undefined ? (
        <p className="muted small">Placing the days...</p>
      ) : (
        <TimelineChart
          chart={chart}
          spec={spec}
          viewKey={step}
          range={undefined}
          onFold={onFold}
          selected={selected}
          title={(key) => (onOpen === undefined ? <span>{titleOf(key)}</span> : <Link to={addresses.open(key)} data-node={key} onClick={(event) => { event.preventDefault(); onOpen(key); }}>{titleOf(key)}</Link>)}
          endTitle={titleOf}
          activate={onOpen}
          anchorNote={
            end === undefined || spec.anchor === undefined
              ? "No final milestone has a day, so the timeline ends after its latest day."
              : `Ends at ${titleOf(end)} on day ${String(spec.anchor.day)}${schedule.final_conditional === true ? " at the earliest, if the branch is taken" : ""}.`
          }
          empty={{ filtered: <NothingMatches />, none: "Nothing in this route has an earliest day yet." }}
          undated="No day yet"
          anyScheduled={schedule.entries.length > 0}
        />
      )}
    </>
  );
}
