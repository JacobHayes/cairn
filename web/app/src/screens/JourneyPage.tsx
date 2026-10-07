// The journey's page: its canvas (C1 to C7; 5.2 replaced the node list that stood in for it),
// what it was derived from (revision, deployment revision, today: the query cache's key), the
// toggles and the way out of a drilled-in container, the stalled surface, and a node's detail
// (5.1) beside the canvas at the journey's address with the node's key. What the canvas shows
// is in the address's query (canvas/settings.ts), so every link keeps it.
import { useMemo } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import { JourneyCanvas } from "../canvas/JourneyCanvas.tsx";
import { KindToggles } from "../canvas/KindToggles.tsx";
import { canvasPath, viewFrom, type CanvasView } from "../canvas/settings.ts";
import { Crumbs, StalledSurface } from "../canvas/Surfaces.tsx";
import type { JourneyView } from "../data/journeys.ts";
import { useJourney } from "../data/react.ts";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import { Badge } from "../ui/kit.tsx";
import { JourneyNav } from "./JourneyNav.tsx";

type Ready = Extract<JourneyView, { status: "ready" }>;

function Header({ ready, view, selected }: { ready: Ready; view: CanvasView; selected: string | undefined }) {
  const navigate = useNavigate();
  const { header } = ready.journey;
  const { key } = ready;
  return (
    <section className="stack" aria-label={header.name}>
      <div className="row">
        <h1 className="title" data-testid="journey-name">{header.name}</h1>
        <Badge>{header.status}</Badge>
        {header.lineage == null ? null : (
          <Link to={`/routes/${header.lineage.route}?version=${String(header.lineage.version)}`} data-testid="lineage">
            Route {header.lineage.route}, version {header.lineage.version}
          </Link>
        )}
        <span className="muted" data-testid="derivation" data-revision={key.revision} data-deployment={key.deployment_revision}>
          Derived in this tab at revision {key.revision}, deployment revision {key.deployment_revision}, for {key.today};{" "}
          {ready.derived.frontier.length} on the frontier.
        </span>
      </div>
      <JourneyNav journey={header.id} current="canvas" node={selected} />
      <KindToggles
        view={view}
        journey
        onChange={(next) => {
          void navigate(canvasPath(header.id, next, selected));
        }}
      />
      <Crumbs ready={ready} view={view} />
      <StalledSurface ready={ready} view={view} />
    </section>
  );
}

/**
 * The page for the journey the address names. Keyed by it, so going to another journey
 * starts every row afresh: its drafts and rejections are that journey's, even where two
 * journeys from one route share node keys.
 */
export function JourneyPage() {
  const { id = "", key: selected } = useParams();
  return <JourneyScreen key={id} id={id} selected={selected} />;
}

function JourneyScreen({ id, selected }: { id: string; selected: string | undefined }) {
  const { search } = useLocation();
  const view = useMemo(() => viewFrom(new URLSearchParams(search)), [search]);
  const journey = useJourney(id);
  switch (journey.status) {
    case "loading":
      return <p className="muted">Deriving the journey...</p>;
    case "missing":
      return <p className="callout" data-testid="journey-missing">This journey does not exist. <Link to="/">All journeys</Link></p>;
    case "failed":
      return <p className="callout callout-bad">The journey could not be read: {journey.message}</p>;
    case "skew":
      return <p className="callout">This journey comes from a newer Cairn; reload to see it.</p>;
    case "ready":
      break;
  }
  return (
    <div className={selected === undefined ? "canvas-page" : "canvas-page canvas-split"}>
      {/* Keyed by journey and node, so every form and rejection in it is that node's. */}
      {selected === undefined ? null : <NodeDetailPanel key={`${id}:${selected}`} view={journey} nodeKey={selected} />}
      <div className="stack">
        <Header ready={journey} view={view} selected={selected} />
        <JourneyCanvas ready={journey} view={view} selected={selected} />
      </div>
    </div>
  );
}
