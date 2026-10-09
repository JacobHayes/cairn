// What each projection draws on the journey's pages, from the screens that already drew them
// (the later tracks redesign each one): NEXT, LIST and CARDS; PLAN, GRAPH, LIST and TIMELINE.
// The graph with DECISIONS on is the decision view (C12), and without it the canvas (C1 to
// C7); its crumbs, stalled surface and, in edit mode, the structure's tools (5.6) are the
// head's (CanvasBars).
import { useMemo } from "react";
import { useLocation, useNavigate } from "react-router";

import { listFrom, nextFrom, nextPath, triageFrom } from "../acting/address.ts";
import { ListBody } from "../acting/ListScreen.tsx";
import { NextList } from "../acting/NextScreen.tsx";
import { TriageBody } from "../acting/TriageScreen.tsx";
import { JourneyAuthoringBar } from "../authoring/JourneyAuthoring.tsx";
import type { useEdgeDrawing } from "../authoring/connect.tsx";
import type { Authored } from "../authoring/target.ts";
import { useProjected } from "../canvas/hooks.ts";
import { JourneyCanvas } from "../canvas/JourneyCanvas.tsx";
import { viewFrom } from "../canvas/settings.ts";
import { Crumbs, StalledSurface } from "../canvas/Surfaces.tsx";
import { DecisionCanvas } from "../decisions/DecisionView.tsx";
import type { Ready } from "../detail/model.ts";
import type { JourneyPage, Projection } from "../journeys/address.ts";
import { TimelineChart } from "../timeline/TimelineView.tsx";

function ProjectedTimeline({ ready, selected }: { ready: Ready; selected: string | undefined }) {
  const { search } = useLocation();
  const { kinds, text, decisions } = useMemo(() => listFrom(new URLSearchParams(search)), [search]);
  const narrowing = useMemo(() => ({ kinds, text, decisions }), [kinds, text, decisions]);
  const timeline = useProjected(ready, { projection: "timeline" });
  if (timeline.error !== undefined) {
    return <p className="callout callout-bad">The timeline could not be read: {timeline.error}</p>;
  }
  if (timeline.value === undefined) {
    return <p className="muted small">Placing the dates...</p>;
  }
  return <TimelineChart ready={ready} timeline={timeline.value} narrowing={narrowing} selected={selected} />;
}

export interface ProjectionProps {
  ready: Ready;
  page: JourneyPage;
  projection: Projection;
  selected: string | undefined;
  /** In edit mode, the journey's structure as authored, and the edge being drawn (5.6). */
  authored: Authored | undefined;
  drawing: ReturnType<typeof useEdgeDrawing>;
}

/** The canvas's own bars, which stay with the head: the structure's tools (edit mode), the breadcrumbs and the stalled surface. */
export function CanvasBars({ ready, authored, drawing }: Pick<ProjectionProps, "ready" | "authored" | "drawing">) {
  const { search } = useLocation();
  const view = useMemo(() => viewFrom(new URLSearchParams(search)), [search]);
  return (
    <>
      {authored === undefined ? null : <JourneyAuthoringBar authored={authored} view={view} drawing={drawing} />}
      <Crumbs ready={ready} view={view} />
      <StalledSurface ready={ready} view={view} />
    </>
  );
}

export function ProjectionBody({ ready, page, projection, selected, authored, drawing }: ProjectionProps) {
  const { search } = useLocation();
  const navigate = useNavigate();
  const params = useMemo(() => new URLSearchParams(search), [search]);
  if (page === "next" && projection === "list") {
    return <NextList view={ready} settings={nextFrom(params)} selected={selected} onSettings={(next) => void navigate(nextPath(ready.journey.header.id, next, selected))} />;
  }
  if (page === "next") {
    return <TriageBody view={ready} settings={triageFrom(params)} selected={selected} />;
  }
  if (projection === "list") {
    // A new query starts from its first page with nothing selected.
    return <ListBody key={search} view={ready} settings={listFrom(params)} />;
  }
  if (projection === "timeline") {
    return <ProjectedTimeline ready={ready} selected={selected} />;
  }
  if (params.get("decisions") === "1") {
    return <DecisionCanvas ready={ready} selected={selected} />;
  }
  return <JourneyCanvas ready={ready} view={viewFrom(params)} selected={selected} onPick={authored === undefined ? undefined : drawing.pick} />;
}
