// C16, E4: the cross-journey "mine" list: what the viewer can do now in each active journey,
// journey by journey (alphabetical; no ranking across journeys), each in that journey's own rank
// with the next list's rows. Selecting a row opens the inspector beside it with its form, so a
// participant who owns work in several journeys can do the day here without opening each. A
// journey where nothing of theirs can be done yet says what is theirs next. Completed
// journeys leave it (B11). The candidates are the active journeys referring to the caller's
// entities (the host's filter, a superset); each is derived in the tab and its own "mine"
// decides (H3: every entity of the caller's counts). Kept current (H6).
import { useMemo } from "react";
import { Link, useParams } from "react-router";

import { DEFAULT_NEXT, nextQueryOf } from "../acting/address.ts";
import { NextForYou } from "../acting/NextForYou.tsx";
import { NextRow } from "../acting/NextRow.tsx";
import "../acting/acting.css";
import { useProjected } from "../canvas/hooks.ts";
import { indexKey, journeyIndex, type JourneySummary } from "../data/reads.ts";
import { useLive, useViewer } from "../data/react.ts";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import type { Ready } from "../detail/model.ts";
import { Inspector } from "../shell/frame.tsx";
import { summaryModel } from "../summary/model.ts";
import { overviewPath, queryOf, DEFAULT_FILTERS } from "./address.ts";
import { useMineCounts, useMineOf, useReportMine, type MineEntry } from "./mine.ts";

/** Where a row of journey `id` opens the inspector, beside Mine. */
const minePath = (id: string, key: string) => `/mine/${id}/nodes/${key}`;

const MINE_NEXT = nextQueryOf({ ...DEFAULT_NEXT, mine: true });

/** The journey's progress in one line. */
function Progress({ view }: { view: Ready }) {
  const summary = useProjected(view, { projection: "status_summary" });
  if (summary.value === undefined) {
    return null;
  }
  const model = summaryModel(view, summary.value);
  return (
    <span className="muted" data-testid="mine-progress">
      {model.inScope - model.remaining} of {model.inScope} done
    </span>
  );
}

/** The viewer's rows in one journey, in that journey's rank, and the line for when none can be done yet. */
function MineRows({ view, entries, selected }: { view: Ready; entries: MineEntry[]; selected: string | undefined }) {
  const request = useMemo(() => ({ projection: "next" as const, query: MINE_NEXT }), []);
  const { value: next } = useProjected(view, request);
  const id = view.journey.header.id;
  const rows = next?.items ?? [];
  return (
    <>
      <ol className="plain-list next-list">
        {rows.map((row) => (
          <NextRow key={row.key} view={view} row={row} to={minePath(id, row.key)} selected={row.key === selected} sort="rank" testId="mine-item" owner={false} />
        ))}
      </ol>
      {next !== undefined && rows.length === 0 ? <NextForYou view={view} entries={entries} nothing={false} /> : null}
    </>
  );
}

function JourneyMine({ summary, report, openJourney }: { summary: JourneySummary; report: (id: string, count: number) => void; openJourney: string | undefined }) {
  const mine = useMineOf(summary.id);
  useReportMine(summary.id, mine, report);
  const { key } = useParams();
  if (mine.status === "loading") {
    return <li className="muted small">Deriving {summary.name}...</li>;
  }
  if (mine.status === "failed") {
    return <li className="callout callout-bad" data-testid="mine-failed">What is yours in {summary.name} could not be read: {mine.message}</li>;
  }
  if (mine.status !== "ready" || mine.entries.length === 0) {
    return null;
  }
  const open = openJourney === summary.id ? key : undefined;
  return (
    <li className="stack mine-journey" data-testid="mine-journey" data-journey={summary.id}>
      <span className="row mine-head">
        <strong>{summary.name}</strong>
        <Progress view={mine.view} />
        <span className="spacer" />
        <Link to={overviewPath(summary.id)}>Open &rsaquo;</Link>
      </span>
      <MineRows view={mine.view} entries={mine.entries} selected={open} />
      {open === undefined ? null : (
        <Inspector focus={`${summary.id}:${open}`}>
          <NodeDetailPanel key={`${summary.id}:${open}`} view={mine.view} nodeKey={open} />
        </Inspector>
      )}
    </li>
  );
}

export function MineScreen() {
  const { id } = useParams();
  const { viewer, failed } = useViewer();
  const query = viewer === undefined ? undefined : queryOf({ ...DEFAULT_FILTERS, mine: true }, viewer.entities);
  const { view } = useLive(indexKey(query), journeyIndex(query));
  const entities = viewer?.entities ?? [];
  const items = view.status === "ready" ? [...view.value].sort((left, right) => left.name.localeCompare(right.name)) : [];
  const { report, none } = useMineCounts(items);
  return (
    <section className="stack" aria-label="Mine" data-testid="mine">
      <h1>Mine</h1>
      <span className="muted">
        What you can do now in each active journey.{" "}
        {viewer !== undefined && entities.length === 0 ? "No entity holds a verified email of yours, so nothing is yours yet." : ""}
      </span>
      {failed === undefined ? null : <p className="callout callout-bad">Who you are could not be read: {failed}</p>}
      {failed === undefined && (viewer === undefined || view.status === "loading") ? <p className="muted small">Reading your journeys...</p> : null}
      {view.status === "ready" && viewer !== undefined && none ? <p className="muted small" data-testid="mine-empty">Nothing in any active journey is yours.</p> : null}
      {view.status === "failed" ? <p className="callout callout-bad">The journeys could not be read: {view.message}</p> : null}
      {view.status === "ready" && viewer !== undefined ? (
        <ul className="plain-list stack mine-list">
          {items.map((summary) => (
            <JourneyMine key={summary.id} summary={summary} report={report} openJourney={id} />
          ))}
        </ul>
      ) : null}
    </section>
  );
}
