// C17: route detail. The route's published versions, newest first, each with the journeys on
// it and which of them have an upgrade available (each journey's own field from the host);
// the draft, opened from the latest version, published, or discarded (A11); retiring, which
// hides the route from new-journey creation only (A19); and its files exported and imported
// (A13). Upgrades start one journey at a time from a journey's overview (5.7 adds the flow).
// Kept current (H6): its own ticks and every journey's.
import { Link, useParams } from "react-router";

import { routeRead, type RouteRead } from "../data/reads.ts";
import { useLive } from "../data/react.ts";
import { dateWords } from "../timeline/model.ts";
import { newJourneyPath, overviewPath } from "../journeys/address.ts";
import { DEFAULT_VIEW } from "../canvas/settings.ts";
import { routeCanvasPath } from "../screens/RouteCanvasPage.tsx";
import { Badge, Button, Panel } from "../ui/kit.tsx";
import { ROUTES_PATH } from "./address.ts";
import { useExport } from "./exporting.ts";
import { upgradable, versionRows, type VersionRow } from "./model.ts";
import { RouteActions } from "./RouteActions.tsx";

function Version({ read, row, latest }: { read: RouteRead; row: VersionRow; latest: number | undefined }) {
  const id = read.route.header.id;
  const exportVersion = useExport(id);
  const startable = read.route.header.retired !== true;
  return (
    <li className="stack" data-testid="version" data-version={row.version}>
      <span className="row">
        <strong>Version {row.version}</strong>
        {row.version === latest ? <Badge tone="good">Latest</Badge> : null}
        <span className="muted small">Published {dateWords(row.publishedAt.slice(0, 10), new Date().toISOString().slice(0, 10))}</span>
        <span className="spacer" />
        <Link to={routeCanvasPath(id, row.version, DEFAULT_VIEW)}>Open</Link>
        <Button onClick={() => void exportVersion(row.version)}>Export</Button>
        {startable ? <Link to={newJourneyPath(id, row.version)} data-testid="start-from-version">Start a journey</Link> : null}
      </span>
      {row.journeys.length === 0 ? <span className="muted small">No journeys on this version.</span> : null}
      <ul className="stack">
        {row.journeys.map((journey) => (
          <li key={journey.id} className="row" data-testid="version-journey" data-journey={journey.id} data-upgrade={journey.upgrade ? "available" : "none"}>
            <Link to={overviewPath(journey.id)}>{journey.name}</Link>
            {journey.upgrade ? <Badge tone="warn" data-testid="upgrade" data-status="available">Upgrade available</Badge> : null}
          </li>
        ))}
      </ul>
    </li>
  );
}

function Detail({ read }: { read: RouteRead }) {
  const { header } = read.route;
  const rows = versionRows(read);
  const latest = rows[0]?.version;
  const waiting = upgradable(rows).length;
  return (
    <Panel aria-label={header.name} data-testid="route-detail">
      <span className="row">
        <h1 data-testid="route-detail-name">{header.name}</h1>
        {header.retired === true ? <Badge data-testid="retired">Retired: hidden from new journeys</Badge> : null}
        <span className="spacer" />
        <Link to={ROUTES_PATH}>All routes</Link>
      </span>
      {header.description == null ? null : <p>{header.description}</p>}
      <span className="muted small" data-testid="route-revision" data-revision={read.route.revision}>
        {rows.length} published {rows.length === 1 ? "version" : "versions"}; {waiting} {waiting === 1 ? "journey has" : "journeys have"} an upgrade available.
      </span>
      <RouteActions route={read.route} />
      <ul className="version-list">
        {rows.map((row) => (
          <Version key={row.version} read={read} row={row} latest={latest} />
        ))}
      </ul>
    </Panel>
  );
}

export function RouteDetailPage() {
  const { id = "" } = useParams();
  const { view } = useLive(`route:${id}`, routeRead(id));
  switch (view.status) {
    case "loading":
      return <p className="muted small">Reading the route...</p>;
    case "missing":
      return <p className="callout">This route does not exist. <Link to={ROUTES_PATH}>All routes</Link></p>;
    case "failed":
      return <p className="callout callout-bad">The route could not be read: {view.message}</p>;
    case "ready":
      return <Detail read={view.value} />;
  }
}
