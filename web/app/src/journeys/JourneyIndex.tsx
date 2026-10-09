// C16: the journey index. Every journey the filters keep (status, lineage route and version,
// "mine", "upgrade available"), each with its status, lineage, and whether an upgrade is
// available, kept current (H6). "Upgrade available" is the host's field on each journey,
// never worked out here. "Mine" keeps the journeys where the caller holds something.
import { Link, useLocation, useNavigate } from "react-router";

import { journeyIndex, indexKey, routeIndex, type JourneySummary, type RouteSummary } from "../data/reads.ts";
import { useLive, useViewer } from "../data/react.ts";
import { Badge } from "../ui/kit.tsx";
import { filtersFrom, indexPath, newJourneyPath, overviewPath, queryOf, type IndexFilters } from "./address.ts";
import { IndexFilterBar } from "./IndexFilterBar.tsx";
import { useMineCounts, useMineOf, useReportMine } from "./mine.ts";
import { routeDetailPath } from "../routes/address.ts";

/** C16: the mark a journey's row carries when its route has a newer version: the host's field. */
export function UpgradeMark({ summary }: { summary: JourneySummary }) {
  return summary.upgrade_available ? (
    <Badge tone="warn" data-testid="upgrade" data-status="available">
      Upgrade available{summary.latest_version == null ? "" : ` (version ${String(summary.latest_version)})`}
    </Badge>
  ) : null;
}

/** A journey's lineage: its route, by name when the route index has it, and version. */
export function LineageCell({ summary, routes }: { summary: JourneySummary; routes: readonly RouteSummary[] }) {
  const lineage = summary.lineage;
  if (lineage == null) {
    return <span className="muted small">Started empty</span>;
  }
  const route = routes.find((each) => each.header.id === lineage.route);
  return (
    <Link to={routeDetailPath(lineage.route)} data-testid="row-lineage">
      {route?.header.name ?? lineage.route}, version {lineage.version}
    </Link>
  );
}

export function JourneyRow({ summary, routes, mine }: { summary: JourneySummary; routes: readonly RouteSummary[]; mine?: number }) {
  return (
    <tr data-testid="journey-row" data-journey={summary.id} data-status={summary.status}>
      <td>
        <Link to={`/journeys/${summary.id}`}>{summary.name}</Link>
        <div className="row">
          <span className="muted mono">{summary.id}</span>
          <Link to={overviewPath(summary.id)} className="muted small" data-testid="row-overview">
            Overview
          </Link>
        </div>
      </td>
      <td>
        <Badge>{summary.status}</Badge>
      </td>
      <td>
        <span className="stack">
          <LineageCell summary={summary} routes={routes} />
          <UpgradeMark summary={summary} />
        </span>
      </td>
      <td className="mono">{mine === undefined ? summary.revision : `${String(mine)} mine`}</td>
    </tr>
  );
}

/** A row kept only when the caller holds something in the journey (E4). */
function MineRow({ summary, routes, report }: { summary: JourneySummary; routes: readonly RouteSummary[]; report: (id: string, count: number) => void }) {
  const mine = useMineOf(summary.id);
  useReportMine(summary.id, mine, report);
  if (mine.status === "failed") {
    return (
      <tr data-testid="journey-row-failed" data-journey={summary.id}>
        <td colSpan={4} className="callout callout-bad">
          What is yours in {summary.name} could not be read: {mine.message}
        </td>
      </tr>
    );
  }
  if (mine.status !== "ready" || mine.entries.length === 0) {
    return null;
  }
  return <JourneyRow summary={summary} routes={routes} mine={mine.entries.length} />;
}

function Rows({ filters, items, routes }: { filters: IndexFilters; items: JourneySummary[]; routes: RouteSummary[] }) {
  const { report, none } = useMineCounts(filters.mine ? items : []);
  if (items.length === 0) {
    return <p className="muted small" data-testid="index-empty">No journeys match.</p>;
  }
  // With "mine", every candidate stays mounted (kept current) though its row may be empty.
  const empty = filters.mine && none;
  return (
    <div className="table-wrap">
      {empty ? <p className="muted small" data-testid="index-empty">No journeys match: nothing in them is yours.</p> : null}
      <table className="data">
        {empty ? null : (
          <thead>
            <tr>
              <th>Journey</th>
              <th>Status</th>
              <th>Lineage</th>
              <th>{filters.mine ? "Yours" : "Revision"}</th>
            </tr>
          </thead>
        )}
        <tbody>
          {items.map((summary) =>
            filters.mine ? <MineRow key={summary.id} summary={summary} routes={routes} report={report} /> : <JourneyRow key={summary.id} summary={summary} routes={routes} />,
          )}
        </tbody>
      </table>
    </div>
  );
}

export function JourneyIndex() {
  const { search } = useLocation();
  const navigate = useNavigate();
  const filters = filtersFrom(new URLSearchParams(search));
  const { viewer, failed } = useViewer();
  const query = queryOf(filters, viewer?.entities ?? []);
  const waiting = filters.mine && viewer === undefined && failed === undefined;
  const { view } = useLive(indexKey(waiting ? undefined : query), journeyIndex(waiting ? undefined : query));
  const routes = useLive("routes", routeIndex).view;
  const known = routes.status === "ready" ? routes.value : [];
  return (
    <section className="stack" aria-label="Journeys">
      <div className="row">
        <h1>Journeys</h1>
        <span className="spacer" />
        <Link className="button" to={newJourneyPath()} data-testid="new-journey">New journey</Link>
      </div>
      <IndexFilterBar filters={filters} routes={known} onChange={(next) => void navigate(indexPath(next))} />
      {filters.mine && viewer === undefined && failed !== undefined ? <p className="callout callout-bad">Who you are could not be read, so neither can what is yours: {failed}</p> : null}
      {view.status === "loading" || waiting ? <p className="muted small">Loading the journeys...</p> : null}
      {view.status === "failed" ? <p className="callout callout-bad">The journeys could not be read: {view.message}</p> : null}
      {view.status === "ready" && !waiting ? <Rows filters={filters} items={view.value} routes={known} /> : null}
    </section>
  );
}
