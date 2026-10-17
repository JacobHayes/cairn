// The routes index (`/routes`): routes and segments in one table, each linking to its detail (C17,
// C19) and saying one thing of itself: a segment how much it is used ("Used in 6 places", or "2 of
// 6 behind"), a route how to start a journey, or that it has a draft open or is retired (A19:
// hidden from new-journey creation, its journeys still upgrading). The `Routes` and `Segments`
// chips keep it to one (both off shows all); `New route` opens the form that starts a route or
// segment empty, to author by hand (A12), or imports a route file as a new route or draft (A13).
// Kept current (H6).
import { Link, useSearchParams } from "react-router";

import { segmentUses, routeIndex, type RouteSummary, type SegmentUse } from "../data/reads.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useLive } from "../data/react.ts";
import { newJourneyPath } from "../journeys/address.ts";
import { Badge } from "../ui/kit.tsx";
import { newRoutePath, routeDetailPath } from "./address.ts";

/** The one secondary fact of a segment: how many places use it, or how many of them are behind. */
export function usageWords(use: SegmentUse | undefined): string {
  if (use === undefined) {
    return "";
  }
  if (use.places === 0) {
    return "Not used yet";
  }
  return use.behind > 0 ? `${String(use.behind)} of ${String(use.places)} behind` : `Used in ${String(use.places)} ${use.places === 1 ? "place" : "places"}`;
}

/** The row's one fact: a segment's use; a route's way to start a journey, or the one thing that sets it apart (an open draft, retirement). */
function Fact({ route, use }: { route: RouteSummary; use: SegmentUse | undefined }) {
  const { header } = route;
  const latest = route.latest_version ?? undefined;
  if (header.kind === "segment") {
    return (
      <span className="muted small" data-testid="segment-use">
        {usageWords(use)}
      </span>
    );
  }
  if (header.retired === true) {
    return <Badge data-testid="retired">Retired</Badge>;
  }
  if (route.draft_open) {
    return <Badge tone="warn">Draft open</Badge>;
  }
  return latest === undefined ? <span className="muted small">None published</span> : <Link to={newJourneyPath(header.id, latest)}>Start a journey</Link>;
}

function RouteRow({ route, use }: { route: RouteSummary; use: SegmentUse | undefined }) {
  const { header } = route;
  const segment = header.kind === "segment";
  return (
    <tr data-testid="route-row" data-route={header.id} data-kind={segment ? "segment" : "process"}>
      <td>
        <Link to={routeDetailPath(header.id)} title={header.description ?? undefined}>
          {header.name}
        </Link>
      </td>
      <td>{segment ? "Segment" : "Route"}</td>
      <td>
        <Fact route={route} use={use} />
      </td>
    </tr>
  );
}

const TYPES = [
  { id: "routes", label: "Routes" },
  { id: "segments", label: "Segments" },
] as const;

/** The chips: each keeps the list to its type, pressed again it lets everything back (both off shows all). */
function TypeChips({ type, onPick }: { type: string | null; onPick: (next: string | undefined) => void }) {
  return (
    <div className="journey-chips" role="group" aria-label="Show" data-testid="routes-types">
      {TYPES.map((each) => (
        <button key={each.id} type="button" className="chip" aria-pressed={type === each.id} data-testid={`type-${each.id}`} onClick={() => { onPick(type === each.id ? undefined : each.id); }}>
          {each.label}
        </button>
      ))}
    </div>
  );
}

export function RouteIndex() {
  const { view } = useLive("routes", routeIndex);
  const uses = useLive("segment-uses", segmentUses).view;
  const [params, setParams] = useSearchParams();
  const type = params.get("type");
  const routes = valueOf(view);
  const shown = routes !== undefined ? routes.filter((route) => type === null || (type === "segments") === (route.header.kind === "segment")) : [];
  return (
    <section className="stack" aria-label="Routes" data-testid="routes">
      <div className="row">
        <h1>Routes</h1>
        <span className="spacer" />
        <Link className="button" to={newRoutePath(type === "segments" ? "segment" : undefined)} data-testid="new-route-link">New route</Link>
      </div>
      <TypeChips type={type} onPick={(next) => { setParams(next === undefined ? {} : { type: next }, { replace: true }); }} />
      {view.status === "idle" || view.status === "loading" ? <p className="muted small">Loading the routes...</p> : null}
      {view.status === "failed" ? <p className="callout callout-bad">The routes could not be read: {view.error.message}</p> : null}
      {routes !== undefined && shown.length === 0 ? <p className="muted small" data-testid="routes-empty">No routes match.</p> : null}
      {routes !== undefined && shown.length > 0 ? (
        <div className="table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Name</th>
                <th>Type</th>
                <th>Use</th>
              </tr>
            </thead>
            <tbody>
              {shown.map((route) => (
                <RouteRow key={route.header.id} route={route} use={valueOf(uses)?.[route.header.id]} />
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  );
}
