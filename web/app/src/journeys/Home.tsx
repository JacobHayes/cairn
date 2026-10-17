// Home (`/`; C16, E4): what the viewer can do now in each active journey, journey by journey
// (alphabetical; no ranking across journeys), each in that journey's own rank with the frontier's
// rows, then the viewer's active journeys as the index's table. It renders in the first paint: the
// headings are there and the reading lines sit under them. A row opens the node's own address (the
// graph, with the node selected); a row's actions (Start, Done, Mark reached, Assign) act in place,
// and Decide opens the node's answer form, which is the inspector's. A journey where nothing of
// theirs can be done yet says what is theirs next. Completed journeys leave it (B11). The
// candidates are the active journeys referring to the caller's entities (the host's filter, a
// superset); each is derived in the tab and its own "mine" decides (H3: every entity of the
// caller's counts). Kept current (H6).
import type { NextQuery } from "@cairn/wasm";
import { useMemo } from "react";
import { Link } from "react-router";

import { NextForYou } from "../acting/NextForYou.tsx";
import { NodeItem, NodePanel } from "../acting/NodeItem.tsx";
import { reasonOf } from "../acting/reasons.ts";
import { RowActions } from "../acting/RowActions.tsx";
import "../acting/acting.css";
import { useProjected } from "../canvas/hooks.ts";
import { valueOf } from "../data/keyed-read.ts";
import { indexKey, journeyIndex, routeIndex, type JourneySummary } from "../data/reads.ts";
import { useLive, useViewer } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { DEFAULT_FILTERS, indexPath, journeyPath, queryOf } from "./address.ts";
import { progressOf, useStatusSummary } from "./figures-model.ts";
import { JourneyTable } from "./JourneyIndex.tsx";
import { useMineCounts, useMineOf, useReportMine, type MineEntry } from "./mine.ts";

const HOME_NEXT: NextQuery = { sort: "rank", mine: true, kinds: [], for_viewer: false };

/** The journey's progress in one line. */
function Progress({ view }: { view: Ready }) {
  const model = useStatusSummary(view);
  if (model === undefined) {
    return null;
  }
  const { done, inScope } = progressOf(model);
  return (
    <span className="muted" data-testid="home-progress">
      {done} of {inScope} done
    </span>
  );
}

/** The viewer's rows in one journey, in that journey's rank, and the line for when none can be done yet. */
function WaitingRows({ view, entries }: { view: Ready; entries: MineEntry[] }) {
  const request = useMemo(() => ({ projection: "next" as const, query: HOME_NEXT }), []);
  const next = valueOf(useProjected(view, request));
  const journey = view.journey.header.id;
  const rows = next?.items ?? [];
  return (
    <>
      {rows.length === 0 ? null : (
        <NodePanel>
          {rows.map((row) => {
            const to = journeyPath({ journey, node: row.key }, "graph");
            return (
              <NodeItem key={row.key} view={view} node={row.key} to={to} fact={reasonOf(view, row, "rank")} testId="home-item">
                <RowActions view={view} row={row} to={to} owner={false} />
              </NodeItem>
            );
          })}
        </NodePanel>
      )}
      {next !== undefined && rows.length === 0 ? <NextForYou view={view} entries={entries} nothing={false} /> : null}
    </>
  );
}

function WaitingJourney({ summary, report }: { summary: JourneySummary; report: (id: string, count: number) => void }) {
  const mine = useMineOf(summary.id);
  useReportMine(summary.id, mine, report);
  if (mine.status === "loading") {
    return <li className="muted small">Deriving {summary.name}...</li>;
  }
  if (mine.status === "failed") {
    return <li className="callout callout-bad" data-testid="home-failed">What is yours in {summary.name} could not be read: {mine.message}</li>;
  }
  if (mine.status !== "ready" || mine.entries.length === 0) {
    return null;
  }
  return (
    <li className="stack home-journey" data-testid="home-journey" data-journey={summary.id}>
      <span className="row home-head">
        <Link to={journeyPath({ journey: summary.id })}>
          <strong>{summary.name}</strong>
        </Link>
        <Progress view={mine.view} />
      </span>
      <WaitingRows view={mine.view} entries={mine.entries} />
    </li>
  );
}

export function Home() {
  const { viewer, failed } = useViewer();
  const query = viewer === undefined ? undefined : queryOf({ ...DEFAULT_FILTERS, mine: true }, viewer.entities);
  const { view } = useLive(indexKey(query), journeyIndex(query));
  const routes = valueOf(useLive("routes", routeIndex).view) ?? [];
  const entities = viewer?.entities ?? [];
  const found = valueOf(view);
  const items = useMemo(() => (found === undefined ? [] : [...found].sort((left, right) => left.name.localeCompare(right.name))), [found]);
  const { report, none } = useMineCounts(items);
  const settled = found !== undefined && viewer !== undefined;
  return (
    <section className="stack home" aria-label="Home" data-testid="home">
      <h1>Waiting on you</h1>
      <span className="muted">
        What you can do now in each active journey.{" "}
        {viewer !== undefined && entities.length === 0 ? "No entity holds a verified email of yours, so nothing is yours yet." : ""}
      </span>
      {failed === undefined ? null : <p className="callout callout-bad">Who you are could not be read: {failed}</p>}
      {failed === undefined && (viewer === undefined || view.status === "loading") ? <p className="muted small">Reading your journeys...</p> : null}
      {view.status === "failed" ? <p className="callout callout-bad">The journeys could not be read: {view.error.message}</p> : null}
      {settled && none ? (
        <p className="muted small" data-testid="home-empty">
          Nothing in any active journey is yours. <Link to={indexPath()}>All journeys &rsaquo;</Link>
        </p>
      ) : null}
      {settled ? (
        <ul className="plain-list stack home-list">
          {items.map((summary) => (
            <WaitingJourney key={summary.id} summary={summary} report={report} />
          ))}
        </ul>
      ) : null}
      {settled && !none ? (
        <>
          <div className="row">
            <h2>Your active journeys</h2>
            <span className="spacer" />
            <Link to={indexPath()}>All journeys &rsaquo;</Link>
          </div>
          <JourneyTable items={items} routes={routes} mine />
        </>
      ) : null}
    </section>
  );
}
