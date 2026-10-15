// What each view draws on the journey's page: the graph, the list, the timeline and the walk
// through.
// The graph with DECISIONS on is the decision view (C12): the same canvas with every card that
// is not a decision faded; its stalled surface and, in edit mode, the
// structure's tools (5.6) are the head's (CanvasBars).
import { lazy, Suspense, useMemo } from "react";
import { useLocation } from "react-router";

import { listFrom, triageFrom } from "../acting/address.ts";
import { ListBody } from "../acting/ListScreen.tsx";
import { TriageBody } from "../acting/TriageScreen.tsx";
import { JourneyAuthoringBar } from "../authoring/JourneyAuthoring.tsx";
import type { useEdgeDrawing } from "../authoring/connect.tsx";
import type { Authored } from "../authoring/target.ts";
import { useProjected } from "../canvas/hooks.ts";
import { viewFrom } from "../canvas/settings.ts";
import { StalledSurface } from "../canvas/Surfaces.tsx";
import type { Selection } from "../canvas/selection.ts";
import type { Ready } from "../detail/model.ts";
import type { Screen } from "../journeys/address.ts";
import { TimelineChart, timelineFrom } from "../timeline/TimelineView.tsx";
import { ViewBar } from "./ViewControls.tsx";

/** The graph and its library load with the graph alone: the other views open without them. */
const JourneyCanvas = lazy(() => import("../canvas/JourneyCanvas.tsx").then((module) => ({ default: module.JourneyCanvas })));

function ProjectedTimeline({ ready, selected }: { ready: Ready; selected: string | undefined }) {
  const { search } = useLocation();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  const { kinds, text, decisions, flags, frontier } = useMemo(() => listFrom(params), [params]);
  const { detail, range } = useMemo(() => timelineFrom(params), [params]);
  const mine = flags.includes("mine");
  const held = useProjected(ready, mine ? { projection: "mine" } : undefined);
  const holding = useMemo(() => (mine ? new Set((held.value ?? []).map((entry) => entry.node)) : undefined), [mine, held.value]);
  const narrow = useMemo(() => ({ kinds, text, decisions, mine: holding, frontier: frontier ? new Set(ready.derived.acting_frontier) : undefined }), [kinds, text, decisions, holding, frontier, ready.derived.acting_frontier]);
  const timeline = useProjected(ready, { projection: "timeline" });
  if (timeline.error !== undefined) {
    return <p className="callout callout-bad">The timeline could not be read: {timeline.error}</p>;
  }
  if (timeline.value === undefined) {
    return <p className="muted small">Placing the dates...</p>;
  }
  return <TimelineChart key={detail} ready={ready} timeline={timeline.value} narrow={narrow} selected={selected} detail={detail} range={range} />;
}

export interface ProjectionProps {
  ready: Ready;
  selection: Selection;
  screen: Exclude<Screen, "summary">;
  selected: string | undefined;
  /** The edge whose card is open, as `<from>~<to>`. */
  edge: string | undefined;
  /** In edit mode, the journey's structure as authored, and the edge being drawn (5.6). */
  authored: Authored | undefined;
  drawing: ReturnType<typeof useEdgeDrawing>;
}

/** The canvas's own bars, which stay with the head: the structure's tools (edit mode) and the stalled surface. */
export function CanvasBars({ ready, authored, drawing }: Pick<ProjectionProps, "ready" | "authored" | "drawing">) {
  const { search } = useLocation();
  const view = useMemo(() => viewFrom(new URLSearchParams(search)), [search]);
  return (
    <>
      {authored === undefined ? null : <JourneyAuthoringBar ready={ready} authored={authored} view={view} drawing={drawing} />}
      <StalledSurface ready={ready} view={view} />
    </>
  );
}

export function ProjectionBody({ ready, screen, selected, edge, authored, drawing, selection }: ProjectionProps) {
  const { search } = useLocation();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  // FRONTIER only highlights the graph's cards that can move now: nothing drawn changes.
  const frontier = params.get("frontier") === "1";
  const emphasis = useMemo(() => (frontier ? new Set(ready.derived.acting_frontier) : null), [frontier, ready.derived.acting_frontier]);
  const bar = <ViewBar ready={ready} screen={screen} node={selected} />;
  if (screen === "walk") {
    return (
      <>
        {bar}
        <TriageBody view={ready} settings={triageFrom(params)} selected={selected} />
      </>
    );
  }
  if (screen === "list") {
    return <ListBody view={ready} settings={listFrom(params)} selected={selected} />;
  }
  if (screen === "timeline") {
    return (
      <>
        {bar}
        <ProjectedTimeline ready={ready} selected={selected} />
      </>
    );
  }
  return (
    <Suspense fallback={<p className="muted small">Drawing the graph...</p>}>
      <JourneyCanvas ready={ready} view={viewFrom(params)} selected={selected} edge={edge} decisions={params.get("decisions") === "1"} emphasis={emphasis} onPick={authored === undefined ? undefined : drawing.pick} selection={selection} />
    </Suspense>
  );
}
