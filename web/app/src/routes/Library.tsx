// The Library: the route index, every route with its latest version, whether a draft is open, and whether
// it is retired (A19: hidden from new-journey creation, its journeys still upgrading), each
// linking to its detail (C17); a route file imported as a new route or draft (A13); and a new
// route started empty, to author by hand (A12). Kept current (H6).
import { Link } from "react-router";

import { routeIndex, type RouteSummary } from "../data/reads.ts";
import { useLive } from "../data/react.ts";
import { newJourneyPath } from "../journeys/address.ts";
import { Badge } from "../ui/kit.tsx";
import { routeDetailPath } from "./address.ts";
import { ImportFile } from "./ImportFile.tsx";
import { NewRoute } from "./NewRoute.tsx";

function RouteRow({ route }: { route: RouteSummary }) {
  const { header } = route;
  const latest = route.latest_version ?? undefined;
  return (
    <tr data-testid="route-row" data-route={header.id}>
      <td>
        <Link to={routeDetailPath(header.id)}>{header.name}</Link>
        <div className="muted mono">{header.id}</div>
      </td>
      <td>{latest === undefined ? <span className="muted small">None published</span> : `Version ${String(latest)}`}</td>
      <td>
        <span className="row">
          {route.draft_open ? <Badge tone="warn">Draft open</Badge> : null}
          {header.retired === true ? <Badge data-testid="retired">Retired</Badge> : null}
        </span>
      </td>
      <td>{header.retired !== true && latest !== undefined ? <Link to={newJourneyPath(header.id, latest)}>Start a journey</Link> : null}</td>
    </tr>
  );
}

export function Library() {
  const { view } = useLive("routes", routeIndex);
  return (
    <section className="stack" aria-label="Library" data-testid="library">
      <h1>Library</h1>
      <h2 data-testid="library-routes">Routes</h2>
      <NewRoute />
      <ImportFile />
      {view.status === "loading" ? <p className="muted small">Loading the routes...</p> : null}
      {view.status === "failed" ? <p className="callout callout-bad">The routes could not be read: {view.message}</p> : null}
      {view.status === "ready" ? (
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Route</th>
                <th>Latest</th>
                <th>State</th>
                <th>New journey</th>
              </tr>
            </thead>
            <tbody>
              {view.value.map((route) => (
                <RouteRow key={route.header.id} route={route} />
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  );
}
