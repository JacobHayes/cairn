// What every acting surface (list, next, triage) sits in: the journey read and derived in the
// tab and kept current (H6), its name, the way to its other screens, and what it was derived
// from (the query cache's key), or why it cannot be shown.
import type { ReactNode } from "react";
import { Link } from "react-router";

import { useJourney } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { JourneyNav, type JourneyScreen } from "../screens/JourneyNav.tsx";
import "./acting.css";

function Derivation({ ready }: { ready: Ready }) {
  const { key } = ready;
  return (
    <span className="muted" data-testid="derivation" data-revision={key.revision} data-deployment={key.deployment_revision} data-today={key.today}>
      Derived in this tab at revision {key.revision} for {key.today}; {ready.derived.acting_frontier.length} to act on now.
    </span>
  );
}

/**
 * Journey `id`'s screen `screen`, its body drawn by `children` once the journey is derived,
 * and anything `header` adds to the heading's row.
 */
export function ActingFrame({ id, screen, children, header }: { id: string; screen: JourneyScreen; children: (ready: Ready) => ReactNode; header?: (ready: Ready) => ReactNode }) {
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
  return (
    <div className="stack acting-page">
      <section className="stack" aria-label={journey.journey.header.name}>
        <div className="row">
          <h1 className="title" data-testid="journey-name">{journey.journey.header.name}</h1>
          <Derivation ready={journey} />
          {header?.(journey)}
        </div>
        <JourneyNav journey={id} current={screen} />
      </section>
      {children(journey)}
    </div>
  );
}
