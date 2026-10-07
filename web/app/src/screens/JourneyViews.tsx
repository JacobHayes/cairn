// The journey's read-mostly screens over its local derivation (ARCHITECTURE, Web UI: Screens):
// the decision view (C12), the timeline (C13), and the status summary (C18), each in the
// journey screen's frame, each a projection the derive worker answers per revision, so each
// follows the journey live (H6).
import { useMemo } from "react";

import { useProjected } from "../canvas/hooks.ts";
import { DecisionView } from "../decisions/DecisionView.tsx";
import type { Ready } from "../detail/model.ts";
import { summaryModel } from "../summary/model.ts";
import { StatusSummaryView } from "../summary/StatusSummary.tsx";
import { TimelineChart } from "../timeline/TimelineView.tsx";
import { JourneyScreen } from "./JourneyScreen.tsx";

export function DecisionViewPage() {
  return <JourneyScreen segment="decisions">{(ready, selected) => <DecisionView ready={ready} selected={selected} />}</JourneyScreen>;
}

function ProjectedTimeline({ ready, selected }: { ready: Ready; selected: string | undefined }) {
  const timeline = useProjected(ready, { projection: "timeline" });
  if (timeline.error !== undefined) {
    return <p className="callout callout-bad">The timeline could not be read: {timeline.error}</p>;
  }
  if (timeline.value === undefined) {
    return <p className="muted">Placing the dates...</p>;
  }
  return <TimelineChart ready={ready} timeline={timeline.value} selected={selected} />;
}

export function TimelinePage() {
  return <JourneyScreen segment="timeline">{(ready, selected) => <ProjectedTimeline ready={ready} selected={selected} />}</JourneyScreen>;
}

function ProjectedSummary({ ready, selected }: { ready: Ready; selected: string | undefined }) {
  const summary = useProjected(ready, { projection: "status_summary" });
  const model = useMemo(() => (summary.value === undefined ? undefined : summaryModel(ready, summary.value)), [ready, summary.value]);
  if (summary.error !== undefined) {
    return <p className="callout callout-bad">The summary could not be read: {summary.error}</p>;
  }
  if (model === undefined) {
    return <p className="muted">Summing up the journey...</p>;
  }
  return <StatusSummaryView ready={ready} model={model} selected={selected} />;
}

export function SummaryPage() {
  return <JourneyScreen segment="summary">{(ready, selected) => <ProjectedSummary ready={ready} selected={selected} />}</JourneyScreen>;
}
