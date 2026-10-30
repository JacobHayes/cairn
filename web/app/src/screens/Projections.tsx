// What each view draws on the journey's page: the graph, the list, the timeline and the walk
// through, each from the journey's one view state (journeys/view.ts) and its own settings.
import { lazy, Suspense, useMemo } from "react";
import { useLocation } from "react-router";

import { listSettingsFrom } from "../acting/address.ts";
import { ListBody } from "../acting/ListScreen.tsx";
import { TriageBody } from "../acting/TriageScreen.tsx";
import type { EdgeDrawing } from "../authoring/connect.tsx";
import type { Authored } from "../authoring/target.ts";
import type { GraphView } from "../canvas/settings.ts";
import type { Selection } from "../canvas/selection.ts";
import type { Ready } from "../detail/model.ts";
import type { Screen } from "../journeys/address.ts";
import { useViewState } from "../journeys/view.ts";
import { ProjectedTimeline } from "../timeline/TimelineView.tsx";
import { Pending } from "../ui/Pending.tsx";

/** The graph and its library load with the graph alone: the other views open without them. */
const JourneyCanvas = lazy(() => import("../canvas/JourneyCanvas.tsx").then((module) => ({ default: module.JourneyCanvas })));

export interface ProjectionProps {
  ready: Ready;
  /** The graph's view, parsed once from the address by the frame. */
  view: GraphView;
  selection: Selection;
  screen: Exclude<Screen, "summary">;
  selected: string | undefined;
  /** The line whose card is open, as `<from>~<to>`; the canvas draws it in the inspector. */
  edge: string | undefined;
  /** In edit mode, the journey's structure as authored, and the edge being drawn. */
  authored: Authored | undefined;
  drawing: EdgeDrawing;
  /** The walk reports the card it is on (none when it has none), so the frame can leave the node it names to the card. */
  onCard: (key: string | undefined) => void;
}

export function ProjectionBody({ ready, view, screen, selected, edge, authored, drawing, selection, onCard }: ProjectionProps) {
  const { search } = useLocation();
  const { state, picks } = useViewState();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  if (screen === "walk") {
    return <TriageBody view={ready} state={state} selected={selected} onCard={onCard} />;
  }
  if (screen === "list") {
    return <ListBody view={ready} state={state} picks={picks} list={listSettingsFrom(params)} selected={selected} selection={selection} />;
  }
  if (screen === "timeline") {
    return <ProjectedTimeline ready={ready} state={state} picks={picks} selected={selected} />;
  }
  return (
    <Suspense fallback={<Pending>Drawing the graph...</Pending>}>
      <JourneyCanvas ready={ready} view={view} selected={selected} edge={edge} onPick={authored === undefined ? undefined : drawing.pick} selection={selection} />
    </Suspense>
  );
}
