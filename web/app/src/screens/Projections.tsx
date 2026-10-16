// What each view draws on the journey's page: the graph, the list, the timeline and the walk
// through, each from the journey's one view state (journeys/view.ts) and its own settings.
// The graph's stalled surface and, in edit mode, the structure's tools (5.6) are the head's
// (CanvasBars).
import { lazy, Suspense, useMemo } from "react";
import { useLocation } from "react-router";

import { listSettingsFrom } from "../acting/address.ts";
import { ListBody } from "../acting/ListScreen.tsx";
import { TriageBody } from "../acting/TriageScreen.tsx";
import { JourneyAuthoringBar } from "../authoring/JourneyAuthoring.tsx";
import type { useEdgeDrawing } from "../authoring/connect.tsx";
import type { Authored } from "../authoring/target.ts";
import type { GraphView } from "../canvas/settings.ts";
import { StalledSurface } from "../canvas/Surfaces.tsx";
import type { Selection } from "../canvas/selection.ts";
import type { Ready } from "../detail/model.ts";
import type { Screen } from "../journeys/address.ts";
import { useViewState } from "../journeys/view.ts";
import { ProjectedTimeline } from "../timeline/TimelineView.tsx";

/** The graph and its library load with the graph alone: the other views open without them. */
const JourneyCanvas = lazy(() => import("../canvas/JourneyCanvas.tsx").then((module) => ({ default: module.JourneyCanvas })));

export interface ProjectionProps {
  ready: Ready;
  /** The graph's view, parsed once from the address by the frame. */
  view: GraphView;
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
export function CanvasBars({ ready, view, authored, drawing }: Pick<ProjectionProps, "ready" | "view" | "authored" | "drawing">) {
  return (
    <>
      {authored === undefined ? null : <JourneyAuthoringBar ready={ready} authored={authored} view={view} drawing={drawing} />}
      <StalledSurface ready={ready} view={view} />
    </>
  );
}

export function ProjectionBody({ ready, view, screen, selected, edge, authored, drawing, selection }: ProjectionProps) {
  const { search } = useLocation();
  const { state, picks } = useViewState();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  if (screen === "walk") {
    return <TriageBody view={ready} state={state} selected={selected} />;
  }
  if (screen === "list") {
    return <ListBody view={ready} state={state} picks={picks} list={listSettingsFrom(params)} selected={selected} selection={selection} />;
  }
  if (screen === "timeline") {
    return <ProjectedTimeline ready={ready} state={state} picks={picks} selected={selected} />;
  }
  return (
    <Suspense fallback={<p className="muted small">Drawing the graph...</p>}>
      <JourneyCanvas ready={ready} view={view} selected={selected} edge={edge} onPick={authored === undefined ? undefined : drawing.pick} selection={selection} />
    </Suspense>
  );
}
