// The routes index (`/routes`): the routes in one table (C17), each linking to its
// detail with how much it is used (its active journeys, counted from their own lineage) and, with a
// published version, the way to start a journey from it. A retired route (A19:
// hidden from new-journey creation) or one with a draft open says so
// in one chip. `New route` opens the
// form that starts a route empty, to author by hand (A12), or imports a route file as
// a new route or draft (A13). Each read says whether it is still going or failed. Kept current (H6).
import { NewDestinations } from "../proposals/NewDestinations.tsx";
import { Link } from "react-router";

import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { indexKey, journeyIndex, routeIndex, type RouteSummary } from "../data/reads.ts";
import { DEFAULT_FILTERS, indexPath, newJourneyPath } from "../journeys/address.ts";
import { IndexHead } from "../ui/IndexHead.tsx";
import { Badge, IndexState } from "../ui/kit.tsx";
import { newRoutePath, routePath } from "./address.ts";
import { activeJourneyCounts, inUseWords, startableVersion } from "./model.ts";
import "../journeys/journeys.css";
import { Failed } from "../ui/Failed.tsx";
import { RetiredBadge } from "./RetiredBadge.tsx";
import { Tip } from "../ui/Tip.tsx";

const ACTIVE = { ...DEFAULT_FILTERS };

/** What the use cell shows: the use, or that it is being read or could not be. */
type UseReading = { kind: "reading" } | { kind: "failed" } | { kind: "known"; count: number };

function UseCell({ route, reading }: { route: RouteSummary; reading: UseReading }) {
  if (reading.kind === "reading") {
    return (
      <td data-column="use" className="muted">
        Reading...
      </td>
    );
  }
  if (reading.kind === "failed") {
    return (
      <td data-column="use">
        <span className="ink-bad">Could not be read</span>
      </td>
    );
  }
  const words = inUseWords(reading.count);
  const nothing = reading.count === 0;
  return (
    <td data-column="use" data-testid="route-use">
      {nothing ? (
        <span className="muted">{words}</span>
      ) : (
        <Link to={indexPath({ ...ACTIVE, route: route.header.id })}>{words}</Link>
      )}
    </td>
  );
}

function RouteRow({ route, reading }: { route: RouteSummary; reading: UseReading }) {
  const { header } = route;
  const start = startableVersion(route);
  const summary = header.description?.split("\n")[0]?.trim();
  return (
    <tr data-testid="route-row" data-route={header.id}>
      <td data-column="name">
        <span className="cell-first">
          <Link to={routePath(header.id)}>{header.name}</Link>
          {header.retired === true ? <RetiredBadge /> : route.draft_open ? <Badge tone="warn">Draft open</Badge> : null}
        </span>
        {summary === undefined || summary === "" ? null : (
          // The row prints the first line only: the whole description is the tip when there is more than that line.
          <Tip tip={header.description?.trim()} kind={(header.description?.trim() ?? "") === summary ? "cut" : "describe"} tap={(header.description?.trim() ?? "") !== summary}>
            <span className="cell-second cell-description">{summary}</span>
          </Tip>
        )}
      </td>
      <UseCell route={route} reading={reading} />
      <td data-column="act">{start === undefined ? null : <Link to={newJourneyPath(header.id, start)}>Start a journey</Link>}</td>
    </tr>
  );
}

/** The routes as a table, each with its active journeys read from `active`. */
function RouteTable({ routes, useOf }: { routes: RouteSummary[]; useOf: (route: RouteSummary) => UseReading }) {
  return (
    <div className="index-table">
      <table className="data">
        <thead>
          <tr>
            <th scope="col">Name</th>
            <th scope="col">In use</th>
            <th scope="col">
              <span className="visually-hidden">Start a journey</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {routes.map((route) => (
            <RouteRow key={route.header.id} route={route} reading={useOf(route)} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function RouteIndex() {
  const { view, refresh } = useLive("routes", routeIndex);
  const journeyQuery = { status: ["active" as const] };
  const active = useLive(indexKey(journeyQuery), journeyIndex(journeyQuery));
  const routes = valueOf(view);
  const shown = [...(routes ?? [])].sort((left, right) => left.header.name.localeCompare(right.header.name));
  const counts = activeJourneyCounts(valueOf(active.view) ?? []);
  const useOf = (route: RouteSummary): UseReading => {
    const value = valueOf(active.view);
    if (value !== undefined) {
      return { kind: "known", count: counts.get(route.header.id) ?? 0 };
    }
    return active.view.status === "failed" ? { kind: "failed" } : { kind: "reading" };
  };
  return (
    <section className="stack index-page" aria-label="Routes" data-testid="routes" data-rows-page tabIndex={-1}>
      <IndexHead
        title="Routes"
        action={
          <Link className="button" to={newRoutePath()} data-testid="new-route-link">
            New route
          </Link>
        }
      />
      <NewDestinations scope="new_routes" />
      {view.status === "idle" || view.status === "loading" ? <IndexState kind="loading">Reading the routes...</IndexState> : null}
      {view.status === "failed" ? (
        <IndexState
          kind="failed"
        >
          <Failed what="The routes" error={view.error}>
            <button type="button" className="ghost" onClick={refresh}>
              Try again
            </button>
          </Failed>
        </IndexState>
      ) : null}
      {routes !== undefined && shown.length === 0 ? (
        <IndexState kind="empty" action={<Link to={newRoutePath()}>New route &rsaquo;</Link>} data-testid="routes-empty">
          No routes yet.
        </IndexState>
      ) : null}
      {routes !== undefined && shown.length > 0 ? <RouteTable routes={shown} useOf={useOf} /> : null}
    </section>
  );
}
