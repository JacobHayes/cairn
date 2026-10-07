// C16: a journey's overview. Its name and description (editable), status, lineage and version
// with whether an upgrade is available (the host's field), its own notes and links (G1), and
// its life: complete (suggested once nothing in scope is left, or the final milestone is
// reached, B11), reopen, archive, un-archive, and hard delete behind its name typed back
// (A19); and the proposals that upgrade it, save it as a route, and re-link it (B7, B8, B9).
// Kept current (H6).
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router";

import { ActingFrame } from "../acting/Frame.tsx";
import { indexKey, journeyIndex, routeIndex } from "../data/reads.ts";
import { useLive, useSession } from "../data/react.ts";
import { AnnotationList } from "../detail/Attachments.tsx";
import type { Ready } from "../detail/model.ts";
import { routeDetailPath } from "../routes/address.ts";
import { JourneyFlows } from "../proposals/Entries.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { Badge, Panel } from "../ui/kit.tsx";
import { UpgradeMark } from "./JourneyIndex.tsx";
import { completionSuggested } from "./lifecycle.ts";
import { HeaderEditor } from "./HeaderEditor.tsx";
import { StatusPanel } from "./StatusPanel.tsx";
import "./journeys.css";

/** C16: the journey's row in the host's index, for the host's "upgrade available". */
function Lineage({ ready }: { ready: Ready }) {
  const { header } = ready.journey;
  const lineage = header.lineage ?? undefined;
  const query = lineage === undefined ? undefined : { route: lineage.route, version: lineage.version };
  const { view } = useLive(indexKey(query), journeyIndex(query));
  const routes = useLive("routes", routeIndex).view;
  if (lineage === undefined) {
    return <span className="muted" data-testid="overview-lineage">Started empty: no route, so no upgrades.</span>;
  }
  const summary = view.status === "ready" ? view.value.find((item) => item.id === header.id) : undefined;
  const route = routes.status === "ready" ? routes.value.find((each) => each.header.id === lineage.route) : undefined;
  return (
    <span className="row" data-testid="overview-lineage">
      <Link to={routeDetailPath(lineage.route)}>
        {route?.header.name ?? lineage.route}, version {lineage.version}
      </Link>
      {summary === undefined ? null : <UpgradeMark summary={summary} />}
    </span>
  );
}

/** B11: whether completion is suggested, from the engine's status summary and the final milestone. */
function useSuggested(ready: Ready): boolean {
  const { deriver } = useSession();
  const [suggested, setSuggested] = useState(false);
  useEffect(() => {
    let live = true;
    deriver.project(ready.journey.header.id, { projection: "status_summary" }).then(
      (summary) => {
        if (live) {
          setSuggested(completionSuggested(ready.journey.graph, ready.derived, summary));
        }
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [deriver, ready]);
  return suggested;
}

function OverviewBody({ ready }: { ready: Ready }) {
  const { header } = ready.journey;
  const suggested = useSuggested(ready);
  const annotations = (ready.journey.graph.state?.annotations ?? []).filter((annotation) => annotation.body.node == null);
  return (
    <div className="stack">
      <Panel aria-label="Overview" data-testid="overview">
        <span className="row">
          <Badge data-testid="overview-status" data-status={header.status}>{header.status}</Badge>
          <Lineage ready={ready} />
          <span className="muted">Started {header.created_on}</span>
        </span>
        {header.description == null ? <span className="muted">No description.</span> : <Markdown text={header.description} data-testid="overview-description" />}
        <HeaderEditor ready={ready} />
      </Panel>
      <StatusPanel ready={ready} suggested={suggested} />
      <JourneyFlows ready={ready} />
      <Panel aria-label="The journey's notes and links">
        <AnnotationList view={ready} node={null} annotations={annotations} summary={String(annotations.length)} />
      </Panel>
    </div>
  );
}

export function Overview() {
  const { id = "" } = useParams();
  return (
    <ActingFrame id={id} screen="overview">
      {(ready) => <OverviewBody ready={ready} />}
    </ActingFrame>
  );
}
