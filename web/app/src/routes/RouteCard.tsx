// C17, C19: a route's card, the inspector's empty state on its page. Every state of the route says
// its description and how many journeys, across all its versions, can upgrade to the latest (a
// link to the journey index filtered to those); a published version's card (`VersionCard`) adds
// when it was published, Start a journey, and the journeys on it with which have an upgrade
// available, or for a segment where the version is inserted. The draft card
// (authoring/RouteDraft.tsx) puts the same facts above what blocks publishing. Kept current
// (H6): the route's own ticks and every journey's.
import type { Schema } from "@cairn/client";
import type { ReactNode } from "react";
import { Link } from "react-router";

import { RowsPanel, type PanelRow } from "../acting/RowsPanel.tsx";
import { journeyIndex, routeIndex, routeRead, type RouteRead } from "../data/reads.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useLive, useToday } from "../data/react.ts";
import { DEFAULT_FILTERS, indexPath, journeyPath, newJourneyPath } from "../journeys/address.ts";
import { dateWords } from "../status/when.ts";
import { routePath } from "./address.ts";
import { upgradable, versionRows, type VersionRow } from "./model.ts";
import { plural } from "../ui/words.ts";

type InsertionUse = Schema<"InsertionUse">;

/** The route `id` with its versions and their journeys; none until read. */
export function useRouteRead(id: string): RouteRead | undefined {
  return valueOf(useLive(`route:${id}`, routeRead(id)).view);
}

/** The route's description, and, for a route, how many of its journeys can upgrade to its latest version. */
export function RouteFacts({ read, children }: { read: RouteRead; children?: ReactNode }) {
  const { header } = read.route;
  const rows = versionRows(read);
  const waiting = header.kind === "segment" ? 0 : upgradable(rows).length;
  const latest = rows[0]?.version;
  if (header.description == null && children == null && (waiting === 0 || latest === undefined)) {
    return null;
  }
  return (
    <div className="stack" data-testid="route-facts">
      {header.description == null ? null : <p data-testid="route-description">{header.description}</p>}
      {children}
      {waiting === 0 || latest === undefined ? null : (
        <p className="small" data-testid="route-upgrades" data-count={waiting}>
          <Link to={indexPath({ ...DEFAULT_FILTERS, route: header.id, upgrade: true })}>
            {plural(waiting, "journey")} can upgrade to version {latest}
          </Link>
        </p>
      )}
    </div>
  );
}

/** The names of the journeys and routes that hold insertions, by id. */
function useHostNames(): (host: Schema<"Domain">) => { name: string; to: string } {
  const journeys = useLive("journeys-named", journeyIndex({})).view;
  const routes = useLive("routes", routeIndex).view;
  const journeyName = new Map((valueOf(journeys) ?? []).map((journey) => [journey.id, journey.name] as const));
  const routeName = new Map((valueOf(routes) ?? []).map((route) => [route.header.id, route.header.name] as const));
  return (host) => {
    if (host === "deployment") {
      return { name: "the deployment", to: "/" };
    }
    return "journey" in host
      ? { name: journeyName.get(host.journey) ?? host.journey, to: journeyPath({ journey: host.journey }) }
      : { name: routeName.get(host.route) ?? host.route, to: routePath(host.route) };
  };
}

/** A route version's journeys, one fact each: whether it can upgrade. */
function journeyRows(row: VersionRow): PanelRow[] {
  return row.journeys.map((journey) => ({
    key: journey.id,
    render: () => (
      <li className="node-row" data-row data-testid="version-journey" data-journey={journey.id} data-upgrade={journey.upgrade ? "available" : "none"}>
        <div className="node-row-main">
          <span className="node-row-text">
            <Link to={journeyPath({ journey: journey.id })} className="node-title" data-row-title>
              {journey.name}
            </Link>
            {journey.upgrade ? <span className="node-fact ink-warn" data-testid="upgrade" data-status="available">Upgrade available</span> : null}
          </span>
        </div>
      </li>
    ),
  }));
}

/** A segment version's insertions, each with the host it is inserted in and whether the host is behind. */
function insertionRows(uses: readonly InsertionUse[], hostOf: ReturnType<typeof useHostNames>): PanelRow[] {
  return uses.map((use) => {
    const host = hostOf(use.host);
    return {
      key: JSON.stringify([use.graph, use.insertion]),
      render: () => (
        <li className="node-row" data-row data-testid="insertion-use" data-upgrade={use.upgrade_available ? "available" : "none"}>
          <div className="node-row-main">
            <span className="node-row-text">
              <Link to={host.to} className="node-title" data-row-title>
                {host.name}
              </Link>
              <span className="node-fact muted">{use.title}</span>
              {use.upgrade_available ? <span className="node-fact ink-warn" data-testid="upgrade" data-status="available">Upgrade available</span> : null}
            </span>
          </div>
        </li>
      ),
    };
  });
}

/** The card of published version `version`: when, who is on it, and (a route) where to start from it. */
export function VersionCard({ read, version }: { read: RouteRead; version: number }) {
  const { header } = read.route;
  const segment = header.kind === "segment";
  const rows = versionRows(read);
  const row = rows.find((each) => each.version === version);
  const hostOf = useHostNames();
  const today = useToday();
  const uses = read.detail.versions.find((each) => each.version === version)?.insertions ?? [];
  return (
    <aside className="detail-panel panel stack version-card" aria-label={`Version ${String(version)}`} data-testid="version-card" data-version={version}>
      <div className="stack detail-lead">
        <h2>Version {version}</h2>
        {row === undefined ? null : <span className="muted small">Published {dateWords(row.publishedAt.slice(0, 10), today)}</span>}
      </div>
      <RouteFacts read={read}>
        {segment || header.retired === true ? null : (
          <Link className="button primary" to={newJourneyPath(header.id, version)} data-testid="start-from-version">
            Start a journey
          </Link>
        )}
      </RouteFacts>
      {segment ? (
        uses.length === 0 ? (
          <p className="muted small">Not inserted anywhere on this version.</p>
        ) : (
          <RowsPanel title="Inserted in" testId="insertions-panel" rows={insertionRows(uses, hostOf)} />
        )
      ) : row === undefined || row.journeys.length === 0 ? (
        <p className="muted small">No journeys are on this version.</p>
      ) : (
        <RowsPanel title={`Journeys on version ${String(version)}`} testId="version-journeys" rows={journeyRows(row)} />
      )}
    </aside>
  );
}
