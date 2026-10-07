// The frame of a journey's read-mostly screens (the decision view, the timeline, the status
// summary): the journey read and derived in the tab and kept current (H6), what to show while
// it is not ready, its name and the tabs between its screens, and a node's detail (5.1)
// beside the screen when the address names one (`/nodes/<key>` after the screen's address).
import type { ReactNode } from "react";
import { Link, useParams } from "react-router";

import { useJourney } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import { Badge } from "../ui/kit.tsx";
import { JourneyNav } from "./JourneyNav.tsx";
import "./screens.css";

export interface JourneyScreenProps {
  /** The screen, which is also its segment of the journey's address (JourneyNav). */
  segment: "decisions" | "timeline" | "summary";
  /** The screen, for the journey once it is derived, with the node whose detail is open. */
  children: (ready: Ready, selected: string | undefined) => ReactNode;
}

export function JourneyScreen({ segment, children }: JourneyScreenProps) {
  const { id = "", key: selected } = useParams();
  const journey = useJourney(id);
  switch (journey.status) {
    case "loading":
      return <p className="muted">Deriving the journey...</p>;
    case "missing":
      return <p className="callout">This journey does not exist. <Link to="/">All journeys</Link></p>;
    case "failed":
      return <p className="callout callout-bad">The journey could not be read: {journey.message}</p>;
    case "skew":
      return <p className="callout">This journey comes from a newer Cairn; reload to see it.</p>;
    case "ready":
      break;
  }
  const { header } = journey.journey;
  return (
    <div className={selected === undefined ? "journey-screen" : "journey-screen screen-split"} data-testid={`screen-${segment}`}>
      {selected === undefined ? null : <NodeDetailPanel view={journey} nodeKey={selected} />}
      <div className="stack">
        <section className="stack screen-head" aria-label={header.name}>
          <div className="row">
            <h1 className="title" data-testid="journey-name">{header.name}</h1>
            <Badge>{header.status}</Badge>
            <span
              className="muted"
              data-testid="derivation"
              data-revision={journey.key.revision}
              data-deployment={journey.key.deployment_revision}
              data-today={journey.key.today}
            >
              As of {journey.key.today}, revision {journey.key.revision}
            </span>
          </div>
          <JourneyNav journey={header.id} current={segment} node={selected} />
        </section>
        {children(journey, selected)}
      </div>
    </div>
  );
}
