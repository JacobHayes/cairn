// C16: the journey index, a table opened in full (DESIGN, Table): each journey with its route and
// version, what needs a look, what is the viewer's, its progress and its next milestone
// (JourneyTable.tsx). It keeps every journey the filters keep (status, lineage route and version,
// "mine", "upgrade available"), by name, kept current (H6). "Mine" keeps the journeys where the
// caller holds something. A read that is still going, one that failed, and an empty answer each
// say so, with the way on.
import { Link, useLocation, useNavigate } from "react-router";

import { valueOf } from "../data/keyed-read.ts";
import { useLive, useCaller } from "../data/react.ts";
import { indexKey, isSegment, journeyIndex, routeIndex } from "../data/reads.ts";
import { NewDestinations } from "../proposals/NewDestinations.tsx";
import { IndexHead } from "../ui/IndexHead.tsx";
import { IndexState } from "../ui/kit.tsx";
import { DEFAULT_FILTERS, filtersFrom, indexPath, newJourneyPath, queryOf, type IndexFilters } from "./address.ts";
import { IndexFilterBar } from "./IndexFilterBar.tsx";
import { JourneyTable } from "./JourneyTable.tsx";
import "./journeys.css";
import { Failed } from "../ui/Failed.tsx";

/** Whether `filters` are the index's stock ones, which show the active journeys. */
function isStock(filters: IndexFilters): boolean {
  return indexPath(filters) === indexPath(DEFAULT_FILTERS);
}

/** What the index says when it keeps no journey, and where to go from there. */
function Nothing({ filters }: { filters: IndexFilters }) {
  if (filters.mine) {
    return (
      <IndexState kind="empty" action={<Link to={indexPath()}>Clear filters &rsaquo;</Link>} data-testid="index-empty">
        Nothing in these journeys is yours.
      </IndexState>
    );
  }
  if (isStock(filters)) {
    return (
      <IndexState
        kind="empty"
        action={
          <>
            <Link to={indexPath({ ...DEFAULT_FILTERS, status: "any" })}>Show every status &rsaquo;</Link>
            <Link to={newJourneyPath()}>New journey &rsaquo;</Link>
          </>
        }
        data-testid="index-empty"
      >
        No active journeys.
      </IndexState>
    );
  }
  return (
    <IndexState kind="empty" action={<Link to={indexPath()}>Clear filters &rsaquo;</Link>} data-testid="index-empty">
      No journeys match these filters.
    </IndexState>
  );
}

export function JourneyIndex() {
  const { search } = useLocation();
  const navigate = useNavigate();
  const filters = filtersFrom(new URLSearchParams(search));
  const { caller, failed } = useCaller();
  const query = queryOf(filters, caller?.identities ?? []);
  const waiting = filters.mine && caller === undefined && failed === undefined;
  const { view, refresh } = useLive(indexKey(waiting ? undefined : query), journeyIndex(waiting ? undefined : query));
  const routes = useLive("routes", routeIndex).view;
  const known = valueOf(routes) ?? [];
  const found = valueOf(view);
  const unknownViewer = filters.mine && caller === undefined && failed !== undefined;
  return (
    <section className="stack index-page" aria-label="Journeys" data-rows-page tabIndex={-1}>
      <IndexHead
        title="Journeys"
        action={
          <Link className="button" to={newJourneyPath()} data-testid="new-journey">
            New journey
          </Link>
        }
      >
        <IndexFilterBar filters={filters} routes={known.filter((route) => !isSegment(route))} onChange={(next) => void navigate(indexPath(next))} />
      </IndexHead>
      <NewDestinations scope="new_journeys" />
      {unknownViewer ? <IndexState kind="failed"><Failed what="Who you are" error={failed} /></IndexState> : null}
      {!unknownViewer && (waiting || view.status === "loading" || view.status === "idle") ? <IndexState kind="loading">Reading the journeys...</IndexState> : null}
      {view.status === "failed" ? (
        <IndexState
          kind="failed"
        >
          <Failed what="The journeys" error={view.error}>
            <button type="button" className="ghost" onClick={refresh}>
              Try again
            </button>
          </Failed>
        </IndexState>
      ) : null}
      {found !== undefined && !waiting ? <JourneyTable items={found} routes={known} mine={filters.mine} none={<Nothing filters={filters} />} /> : null}
    </section>
  );
}
