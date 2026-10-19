// The routes index (`/routes`): routes and segments in one table (C17, C19), each linking to its
// detail with its latest version and how much it is used: a route its active journeys (counted
// from their own lineage), a segment its places ("Used in 6 places", or "2 of 6 behind"), and a
// route with a published version the way to start a journey from it. A retired route (A19:
// hidden from new-journey creation, its journeys still upgrading) or one with a draft open says so
// in one chip. The `All | Routes | Segments` control keeps it to one type; `New route` opens the
// form that starts a route or segment empty, to author by hand (A12), or imports a route file as
// a new route or draft (A13). Each read says whether it is still going or failed. Kept current (H6).
import { NewDestinations } from "../proposals/NewDestinations.tsx";
import { Link, useSearchParams } from "react-router";

import { valueOf, type ReadView } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { indexKey, journeyIndex, routeIndex, segmentUses, type RouteSummary } from "../data/reads.ts";
import { DEFAULT_FILTERS, indexPath, newJourneyPath } from "../journeys/address.ts";
import { Badge, IndexState, Segmented } from "../ui/kit.tsx";
import { newRoutePath, routePath } from "./address.ts";
import { activeJourneyCounts, inUseWords, ofType, startableVersion, typeOf, type InUse, type RouteType } from "./model.ts";
import "../journeys/journeys.css";
import { ReadFailed } from "../ui/ReadFailed.tsx";

const TYPES = [
  { value: "all", label: "All" },
  { value: "routes", label: "Routes" },
  { value: "segments", label: "Segments" },
] as const satisfies readonly { value: RouteType; label: string }[];

const ACTIVE = { ...DEFAULT_FILTERS };

/** What the use cell shows: the use, or that it is being read or could not be. */
type UseReading = { kind: "reading" } | { kind: "failed"; message: string } | { kind: "known"; use: InUse };

function UseCell({ route, reading }: { route: RouteSummary; reading: UseReading }) {
  const segment = route.header.kind === "segment";
  if (reading.kind === "reading") {
    return (
      <td data-column="use" className="muted">
        Reading...
      </td>
    );
  }
  if (reading.kind === "failed") {
    return (
      <td data-column="use" title={reading.message}>
        <span className="flag-bad">Could not be read</span>
      </td>
    );
  }
  const words = inUseWords(reading.use);
  const nothing = reading.use.kind === "journeys" ? reading.use.count === 0 : reading.use.places === 0;
  const behind = reading.use.kind === "places" && reading.use.behind > 0;
  return (
    <td data-column="use" data-testid={segment ? "segment-use" : "route-use"}>
      {reading.use.kind === "journeys" && !nothing ? (
        <Link to={indexPath({ ...ACTIVE, route: route.header.id })}>{words}</Link>
      ) : (
        <span className={nothing ? "muted" : behind ? "flag-warn" : undefined}>{words}</span>
      )}
    </td>
  );
}

function RouteRow({ route, reading }: { route: RouteSummary; reading: UseReading }) {
  const { header } = route;
  const segment = header.kind === "segment";
  const start = startableVersion(route);
  const summary = header.description?.split("\n")[0]?.trim();
  return (
    <tr data-testid="route-row" data-route={header.id} data-kind={segment ? "segment" : "process"}>
      <td data-column="name">
        <span className="cell-first">
          <Link to={routePath(header.id)}>{header.name}</Link>
          {header.retired === true ? <Badge data-testid="retired">Retired</Badge> : route.draft_open ? <Badge tone="warn">Draft open</Badge> : null}
        </span>
        {summary === undefined || summary === "" ? null : (
          <span className="cell-second cell-description" title={header.description ?? undefined}>
            {summary}
          </span>
        )}
      </td>
      <td data-column="type">{segment ? "Segment" : "Route"}</td>
      <td data-column="latest" className="mono">
        {route.latest_version == null ? <span className="muted">No version yet</span> : `Version ${String(route.latest_version)}`}
      </td>
      <UseCell route={route} reading={reading} />
      <td data-column="act">{start === undefined ? null : <Link to={newJourneyPath(header.id, start)}>Start a journey</Link>}</td>
    </tr>
  );
}

/** What a read shows for the table's use column. */
function readingOf<T>(view: ReadView<T>, known: (value: T) => InUse): UseReading {
  const value = valueOf(view);
  if (value !== undefined) {
    return { kind: "known", use: known(value) };
  }
  return view.status === "failed" ? { kind: "failed", message: view.error.message } : { kind: "reading" };
}

/** The routes and segments as a table, each with its use as `useOf` reads it. */
function RouteTable({ routes, useOf }: { routes: RouteSummary[]; useOf: (route: RouteSummary) => UseReading }) {
  return (
    <div className="index-table">
      <table className="data">
        <thead>
          <tr>
            <th scope="col">Name</th>
            <th scope="col">Type</th>
            <th scope="col">Latest</th>
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
  const [params, setParams] = useSearchParams();
  const type = typeOf(params.get("type"));
  const { view, refresh } = useLive("routes", routeIndex);
  const journeyQuery = { status: ["active" as const] };
  const active = useLive(type === "segments" ? undefined : indexKey(journeyQuery), journeyIndex(journeyQuery));
  const uses = useLive(type === "routes" ? undefined : "segment-uses", segmentUses);
  const routes = valueOf(view);
  const shown = (routes ?? []).filter((route) => ofType(route, type)).sort((left, right) => left.header.name.localeCompare(right.header.name));
  const counts = activeJourneyCounts(valueOf(active.view) ?? []);
  const useOf = (route: RouteSummary): UseReading =>
    route.header.kind === "segment"
      ? readingOf(uses.view, (all) => ({ kind: "places", places: all[route.header.id]?.places ?? 0, behind: all[route.header.id]?.behind ?? 0 }))
      : readingOf(active.view, () => ({ kind: "journeys", count: counts.get(route.header.id) ?? 0 }));
  const newWords = type === "segments" ? "New segment" : "New route";
  return (
    <section className="stack index-page" aria-label="Routes" data-testid="routes" data-rows-page tabIndex={-1}>
      <div className="row index-head">
        <h1>Routes</h1>
        <span className="spacer" />
        <Link className="button" to={newRoutePath(type === "segments" ? "segment" : undefined)} data-testid="new-route-link">
          {newWords}
        </Link>
      </div>
      <NewDestinations scope="new_routes" segments={type === "all" ? undefined : type === "segments"} />
      <Segmented
        label="Show"
        value={type}
        options={TYPES}
        onChange={(next) => { setParams(next === "all" ? {} : { type: next }, { replace: true }); }}
        data-testid="routes-types"
      />
      {view.status === "idle" || view.status === "loading" ? <IndexState kind="loading">Reading the routes...</IndexState> : null}
      {view.status === "failed" ? (
        <IndexState
          kind="failed"
          action={
            <button type="button" className="ghost" onClick={refresh}>
              Try again
            </button>
          }
        >
          <ReadFailed what="The routes" error={view.error} />
        </IndexState>
      ) : null}
      {routes !== undefined && shown.length === 0 ? (
        <IndexState kind="empty" action={<Link to={newRoutePath(type === "segments" ? "segment" : undefined)}>{newWords} &rsaquo;</Link>} data-testid="routes-empty">
          {type === "segments" ? "No segments yet." : "No routes yet."}
        </IndexState>
      ) : null}
      {routes !== undefined && shown.length > 0 ? <RouteTable routes={shown} useOf={useOf} /> : null}
    </section>
  );
}
