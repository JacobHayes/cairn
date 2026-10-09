// C19: a segment's detail, the route detail's layout: the header word "Segment" with its name,
// `Edit draft` or `Open draft` as the one primary, and the `⋯` holding Export, Import into
// draft and Retire (a segment is inserted, never started, so there is no "Start a journey").
// Each published version, newest first, lists where it is inserted: the host (a journey or a
// route), the insertion's title, and an "Upgrade available" chip when the host is behind.
import type { Schema } from "@cairn/client";
import { Link, useNavigate } from "react-router";

import { DEFAULT_VIEW } from "../canvas/settings.ts";
import type { RouteRead } from "../data/reads.ts";
import { journeyIndex, routeIndex } from "../data/reads.ts";
import { useLive } from "../data/react.ts";
import { overviewPath } from "../journeys/address.ts";
import { Menu } from "../screens/Menu.tsx";
import { Refused } from "../screens/Refused.tsx";
import { routeCanvasPath } from "../screens/RouteCanvasPage.tsx";
import { useScreenWrite } from "../screens/write.ts";
import { dateWords } from "../timeline/model.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { routeDetailPath } from "./address.ts";
import { DetailHeader } from "./DetailHeader.tsx";
import { useExport } from "./exporting.ts";
import { ImportFile } from "./ImportFile.tsx";
import { openDraft, retireMutation, routeMoves, versionRows } from "./model.ts";

type InsertionUse = Schema<"InsertionUse">;

/** The names of the journeys and routes that hold insertions, by id. */
function useHostNames(): (host: Schema<"Domain">) => { name: string; to: string } {
  const journeys = useLive("journeys-named", journeyIndex({})).view;
  const routes = useLive("routes", routeIndex).view;
  const journeyName = new Map(journeys.status === "ready" ? journeys.value.map((journey) => [journey.id, journey.name]) : []);
  const routeName = new Map(routes.status === "ready" ? routes.value.map((route) => [route.header.id, route.header.name]) : []);
  return (host) => {
    if (host === "deployment") {
      return { name: "the deployment", to: "/" };
    }
    return "journey" in host
      ? { name: journeyName.get(host.journey) ?? host.journey, to: overviewPath(host.journey) }
      : { name: routeName.get(host.route) ?? host.route, to: routeDetailPath(host.route) };
  };
}

function Uses({ uses, hostOf }: { uses: readonly InsertionUse[]; hostOf: ReturnType<typeof useHostNames> }) {
  if (uses.length === 0) {
    return <span className="muted small">Not inserted anywhere on this version.</span>;
  }
  return (
    <ul className="stack">
      {uses.map((use) => {
        const host = hostOf(use.host);
        return (
          <li key={JSON.stringify([use.graph, use.insertion])} className="row" data-testid="insertion-use" data-upgrade={use.upgrade_available ? "available" : "none"}>
            <span>
              {use.title} in <Link to={host.to}>{host.name}</Link>
            </span>
            {use.upgrade_available ? <Badge tone="warn" data-testid="upgrade" data-status="available">Upgrade available</Badge> : null}
          </li>
        );
      })}
    </ul>
  );
}

function Header({ read }: { read: RouteRead }) {
  const { route } = read;
  const id = route.header.id;
  const write = useScreenWrite();
  const navigate = useNavigate();
  const exporting = useExport(id);
  const moves = routeMoves(route);
  const drafting = route.draft != null;
  const draftPath = routeCanvasPath(id, undefined, DEFAULT_VIEW);
  return (
    <>
      <DetailHeader header={route.header}>
        {drafting ? (
          <Link className="button primary" to={draftPath} data-testid="open-draft-link">
            Open draft
          </Link>
        ) : (
          <Button primary disabled={write.disabled} data-testid="edit-draft" onClick={() => { void write.run({ target: { route: id }, baseRevision: route.revision, mutations: [openDraft()] }).then((landed) => { if (landed) { void navigate(draftPath); } }); }}>
            Edit draft
          </Button>
        )}
        <Menu label="Segment actions" testId="segment-menu" align="end" trigger={<span aria-hidden="true">⋯</span>}>
          {(close) => (
            <>
              <button type="button" role="menuitem" className="menu-item" onClick={() => { close(); void exporting(drafting ? undefined : Math.max(0, ...(route.versions ?? []))); }}>
                {drafting ? "Export the draft" : "Export"}
              </button>
              <ImportFile label="Import into draft" onImported={close} />
              <button type="button" role="menuitem" className="menu-item" disabled={write.disabled} data-testid="retire" onClick={() => { close(); void write.run({ target: { route: id }, baseRevision: route.revision, mutations: [retireMutation(moves.retire)] }); }}>
                {moves.retire ? "Retire" : "Bring back"}
              </button>
            </>
          )}
        </Menu>
      </DetailHeader>
      {write.rejected === undefined ? null : <Refused rejection={write.rejected} onDismiss={write.dismiss} />}
    </>
  );
}

export function SegmentDetail({ read }: { read: RouteRead }) {
  const { header } = read.route;
  const rows = versionRows(read);
  const latest = rows[0]?.version;
  const insertions = new Map(read.detail.versions.map((version) => [version.version, version.insertions ?? []]));
  const hostOf = useHostNames();
  return (
    <section className="panel stack" aria-label={header.name} data-testid="route-detail" data-kind="segment">
      <Header read={read} />
      {header.description == null ? null : <p>{header.description}</p>}
      <ul className="version-list">
        {rows.map((row) => (
          <li key={row.version} className="stack" data-testid="version" data-version={row.version}>
            <span className="row">
              <strong>Version {row.version}</strong>
              {row.version === latest ? <Badge tone="good">Latest</Badge> : null}
              <span className="muted small">Published {dateWords(row.publishedAt.slice(0, 10), new Date().toISOString().slice(0, 10))}</span>
              <span className="spacer" />
              <Link to={routeCanvasPath(header.id, row.version, DEFAULT_VIEW)}>Open</Link>
            </span>
            <Uses uses={insertions.get(row.version) ?? []} hostOf={hostOf} />
          </li>
        ))}
      </ul>
    </section>
  );
}
