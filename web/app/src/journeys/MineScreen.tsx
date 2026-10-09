// C16, E4: the cross-journey "mine" list: the unranked union of each active journey's "mine"
// (cross-journey ranking is Later), journey by journey, each node with the participation
// kinds the caller holds on it and a link to its detail. Completed journeys leave it (B11).
// The candidates are the active journeys referring to the caller's entities (the host's
// filter, a superset); each is derived in the tab and its own "mine" decides (H3: every
// entity of the caller's counts). Kept current (H6).
import { Link } from "react-router";

import { indexKey, journeyIndex, type JourneySummary } from "../data/reads.ts";
import { useLive, useViewer } from "../data/react.ts";
import { kindTitle } from "../detail/sections.tsx";
import { deepLinkPath, overviewPath, queryOf, DEFAULT_FILTERS } from "./address.ts";
import { useMineCounts, useMineOf, useReportMine } from "./mine.ts";

function JourneyMine({ summary, report }: { summary: JourneySummary; report: (id: string, count: number) => void }) {
  const mine = useMineOf(summary.id);
  useReportMine(summary.id, mine, report);
  if (mine.status === "loading") {
    return <li className="muted small">Deriving {summary.name}...</li>;
  }
  if (mine.status === "failed") {
    return <li className="callout callout-bad" data-testid="mine-failed">What is yours in {summary.name} could not be read: {mine.message}</li>;
  }
  if (mine.status !== "ready" || mine.entries.length === 0) {
    return null;
  }
  const nodes = new Map((mine.view.journey.graph.nodes ?? []).map((node) => [node.key, node]));
  return (
    <li className="stack" data-testid="mine-journey" data-journey={summary.id}>
      <span className="row">
        <Link to={overviewPath(summary.id)}>
          <strong>{summary.name}</strong>
        </Link>
        <span className="muted small">{mine.entries.length} yours</span>
      </span>
      <ul className="stack">
        {mine.entries.map((entry) => (
          <li key={entry.node} className="row" data-testid="mine-item" data-node={entry.node}>
            <Link to={deepLinkPath(summary.id, entry.node)}>{nodes.get(entry.node)?.title ?? entry.node}</Link>
            <span className="muted small">{nodes.get(entry.node)?.kind}</span>
            <span className="muted small">{entry.kinds.map((kind) => kindTitle(mine.view, kind)).join(", ")}</span>
          </li>
        ))}
      </ul>
    </li>
  );
}

export function MineScreen() {
  const { viewer, failed } = useViewer();
  const query = viewer === undefined ? undefined : queryOf({ ...DEFAULT_FILTERS, mine: true }, viewer.entities);
  const { view } = useLive(indexKey(query), journeyIndex(query));
  const entities = viewer?.entities ?? [];
  const items = view.status === "ready" ? view.value : [];
  const { report, none } = useMineCounts(items);
  return (
    <section className="stack" aria-label="Mine" data-testid="mine">
      <h1>Mine, across journeys</h1>
      <span className="muted small">
        What you hold in each active journey, not ranked across journeys.{" "}
        {viewer !== undefined && entities.length === 0 ? "No entity holds a verified email of yours, so nothing is yours yet." : ""}
      </span>
      {failed === undefined ? null : <p className="callout callout-bad">Who you are could not be read: {failed}</p>}
      {failed === undefined && (viewer === undefined || view.status === "loading") ? <p className="muted small">Reading your journeys...</p> : null}
      {view.status === "ready" && viewer !== undefined && none ? <p className="muted small" data-testid="mine-empty">Nothing in any active journey is yours.</p> : null}
      {view.status === "failed" ? <p className="callout callout-bad">The journeys could not be read: {view.message}</p> : null}
      {view.status === "ready" && viewer !== undefined ? (
        <ul className="version-list">
          {view.value.map((summary) => (
            <JourneyMine key={summary.id} summary={summary} report={report} />
          ))}
        </ul>
      ) : null}
    </section>
  );
}
