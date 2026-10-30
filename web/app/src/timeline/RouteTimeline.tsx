// C13: a route's timeline: the route read as a journey that starts on day 0 and nothing is
// waiting on, so each node sits at the earliest day its dependencies, estimates, date rules
// allow. Rows are the nodes the level draws (Depth, Show conditional and the search
// every view of a route shares), under the containment tree and folded by the ladder; the chart
// is the journey timeline's (`TimelineChart`) on a relative axis, with a start line at day 0 and
// the final milestone as the end anchor. The bars are the before and after order; the relations
// themselves are on the Graph. A node with no earliest day gathers in a folded list, and a row
// whose date rule waits on a date decision says so. A row's title opens the node's form on a
// draft, and the node of a version is selected and read.
import type { RouteSchedule } from "@cairn/wasm";
import { useMemo } from "react";
import { useNavigate } from "react-router";

import { titleIn, treeOf } from "../authoring/graph.ts";
import { useFolds } from "../canvas/hooks.ts";

import { errorWords, valueOf, type ReadView } from "../data/keyed-read.ts";
import { useNodeAddresses } from "../journeys/address.ts";

import type { ViewState } from "../journeys/view.ts";
import { nodesOf, type Derived, type Shown } from "../screens/route-reads.ts";
import { NothingMatches, routeSubject, ViewBar } from "../screens/ViewControls.tsx";
import { routeSpec } from "./model.ts";
import { routeChartOf } from "./rows.ts";
import { TimelineChart } from "./TimelineView.tsx";

import { Failed } from "../ui/Failed.tsx";
import { NodeLink } from "../detail/parts.tsx";
import { useViewLevel } from "../canvas/viewLevel.ts";
import { useLayer } from "../proposals/layer.ts";


export interface RouteTimelineProps {
  /** The schedule of the shown graph, read once by the page. */
  schedule: ReadView<RouteSchedule>;
  shown: Derived<Shown>;
  state: ViewState;
  /** How many times a depth was picked: a pick drops the viewer's folds. */
  picks: number;
  selected: string | undefined;
  /** What opening a node does: selects it, or picks it as the end of a requirement being drawn. */
  onOpen: (key: string) => void;
}

export function RouteTimeline({ shown, schedule: scheduleRead, state, picks, selected, onOpen }: RouteTimelineProps) {
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const nodes = nodesOf(shown);
  const tree = useMemo(() => treeOf(shown.graph), [shown.graph]);
  const { step, current, level, error: levelError, dimmed: faded, badges } = useViewLevel({ screen: "timeline", state, nodes, selected, held: shown });
  const layered = useLayer();
  const schedule = valueOf(scheduleRead);
  const { open, fold: onFold } = useFolds(nodes, current, state.depth, step, picks, selected, (key) => void navigate(addresses.open(key)));
  const error = levelError ?? errorWords(scheduleRead);
  const spec = useMemo(() => (schedule === undefined ? undefined : routeSpec(schedule)), [schedule]);
  const chart = useMemo(() => (schedule === undefined || level === undefined ? undefined : routeChartOf(tree, schedule, { drawn: level.nodes, open, selected, faded })), [tree, schedule, level, open, selected, faded]);
  const layer = useMemo(() => {
    if (layered?.mode !== "reviewing" || layered.model === undefined) {
      return undefined;
    }
    const was = layered.beforeShown === undefined ? [] : nodesOf(layered.beforeShown);
    return { marks: layered.model.marks, removed: [...layered.removed].map((key) => ({ key, title: was.find((node) => node.key === key)?.title ?? key })) };
  }, [layered]);
  const titleOf = (key: string) => titleIn(tree, key);
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
          title={(key) => <NodeLink node={key} title={titleOf(key)} onClick={(event) => { event.preventDefault(); onOpen(key); }} />}
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
          layer={layer}
        />
      )}
    </>
  );
}
