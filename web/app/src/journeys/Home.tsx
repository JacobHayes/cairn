// Home (`/`; C16, E4): what the viewer can move now, journey by journey (alphabetical; no ranking
// across journeys), each in that journey's own rank and bounded to five rows with the rest one
// click away; then the journeys where their work is not ready yet, then a link to every journey. It renders in the first paint: the title is there and the reading
// line sits under it, and a journey's group appears as it is read. A row opens the node's own address; its
// actions (Start, Done, Mark reached, Assign) act in place, and Decide opens the node's answer
// form, which is the inspector's. The candidates are the active journeys referring to the
// caller's identities (the host's filter, a superset); each is read in the tab and its own "mine"
// decides (H3: every identity of the caller's counts). Kept current (H6).
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router";

import { pathTo } from "../address.ts";
import { NodeItem, NodePanel } from "../acting/NodeItem.tsx";
import { factOf } from "../acting/reasons.ts";
import { RowActions } from "../acting/RowActions.tsx";
import { RowsPanel, type PanelRow } from "../acting/RowsPanel.tsx";
import { nextForYou, waitsOnWords } from "../acting/yours.ts";
import "../acting/acting.css";
import { valueOf } from "../data/keyed-read.ts";
import { useLive, useRetryJourney, useViewer } from "../data/react.ts";
import { indexKey, journeyIndex, type JourneySummary } from "../data/reads.ts";
import { destinationWords, ProposalRow } from "../proposals/ProposalRows.tsx";
import { listKey, proposalsListed, type Proposals } from "../proposals/read.ts";
import { DEFAULT_FILTERS, indexPath, journeyPath, newJourneyPath, queryOf } from "./address.ts";
import { heldBack, homeState, readyLine, type HomeJourney, type HomeState } from "./home-model.ts";
import { useJourneyReading, type JourneyReading } from "./reading.ts";
import { listPath } from "./view.ts";
import "./journeys.css";
import { Failed } from "../ui/Failed.tsx";
import { plural } from "../ui/words.ts";

type Ready = Extract<JourneyReading, { status: "ready" }>;

/** Reads journey `id` and hands what it read up to the page, which draws it: nothing of its own is drawn. */
function Reader({ id, report }: { id: string; report: (id: string, reading: JourneyReading) => void }) {
  const reading = useJourneyReading(id, false);
  useEffect(() => {
    report(id, reading);
  }, [id, reading, report]);
  return null;
}

/** A journey whose reading failed: its name, the short words, why on request, and the way to try again. */
function FailedGroup({ summary, message }: { summary: JourneySummary; message: string }) {
  const retry = useRetryJourney();
  return (
    <li className="home-journey" data-testid="home-failed" data-journey={summary.id}>
      <NodePanel title={<Link to={journeyPath({ journey: summary.id })}>{summary.name}</Link>} label={summary.name}>
        <li className="home-failed-row">
          <Failed what="This journey" error={message}>
            <button type="button" className="ghost" onClick={() => { retry(summary.id); }}>
              Try again
            </button>
          </Failed>
        </li>
      </NodePanel>
    </li>
  );
}

/** One journey's group: the viewer's rows that can move now, in the journey's rank, and what of theirs is held back. */
function Group({ summary, reading }: { summary: JourneySummary; reading: Ready }) {
  const { view, ready, mine } = reading;
  const journey = summary.id;
  const rows: PanelRow[] = ready.map((row) => ({
    key: `${journey}/${row.key}`,
    render: (shown) => {
      const to = journeyPath({ journey, node: row.key }, "graph");
      const said = factOf(view, row);
      return (
        <NodeItem view={view} node={row.key} to={to} chip={false} fact={said?.words} factTone={said?.tone} testId="home-item">
          {shown ? <RowActions view={view} row={row} to={to} owner={false} /> : null}
        </NodeItem>
      );
    },
  }));
  const held = heldBack(view, mine, ready);
  return (
    <li className="home-journey" data-testid="home-journey" data-journey={journey}>
      <RowsPanel
        title={<Link to={journeyPath({ journey })}>{summary.name}</Link>}
        label={summary.name}
        aside={`${String(ready.length)} ready for you`}
        testId="home-group"
        rows={rows}
        foot={
          held.count === 0 ? undefined : (
            <Link to={listPath(journey, { mine: true })} data-testid="home-held">
              {held.count} more of yours not ready yet
              {held.late === 0 ? null : <span className="ink-bad">, {held.late} late</span>} &rsaquo;
            </Link>
          )
        }
      />
    </li>
  );
}

/** The journeys where the viewer has open work and none of it can move yet: each names their next item and what it waits on. */
function NotReady({ entries, folded }: { entries: { summary: JourneySummary; reading: Ready }[]; folded: boolean }) {
  const rows: PanelRow[] = entries.flatMap(({ summary, reading }) => {
    const next = nextForYou(reading.view, reading.mine);
    if (next === undefined) {
      return [];
    }
    const waits = waitsOnWords(reading.view, next);
    return [
      {
        key: `${summary.id}/${next.node}`,
        render: () => (
          <NodeItem view={reading.view} node={next.node} to={journeyPath({ journey: summary.id, node: next.node }, "graph")} fact={waits === "" ? summary.name : `${summary.name}: ${waits}`} testId="home-waiting-item" />
        ),
      },
    ];
  });
  return rows.length === 0 ? null : <RowsPanel title="Not ready yet" testId="home-waiting" rows={rows} folded={folded} />;
}

/** The panel Home shows in place of the groups when there is nothing to do or nobody to do it for. */
function Statement({ title, children, testId }: { title: string; children: ReactNode; testId: string }) {
  return (
    <section className="panel stack home-statement" aria-label={title} data-testid={testId}>
      <p className="home-statement-title">{title}</p>
      {children}
    </section>
  );
}

/** What Home reads: the viewer, the candidate journeys, and each journey's reading as it settles. */
function useHome() {
  const { viewer, failed } = useViewer();
  const query = viewer === undefined ? undefined : queryOf({ ...DEFAULT_FILTERS, mine: true }, viewer.identities);
  const { view, refresh } = useLive(indexKey(query), journeyIndex(query));
  const found = valueOf(view);
  const proposals = useLive(listKey("waiting"), proposalsListed("waiting"));
  const waitingProposals = valueOf(proposals.view);
  const items = useMemo(() => (found === undefined ? [] : [...found].sort((left, right) => left.name.localeCompare(right.name))), [found]);
  const [readings, setReadings] = useState<Record<string, JourneyReading>>({});
  const report = useCallback((id: string, reading: JourneyReading) => {
    setReadings((held) => (held[id] === reading ? held : { ...held, [id]: reading }));
  }, []);
  const known = items.map((summary) => ({ summary, reading: readings[summary.id] ?? ({ status: "loading" } as const) }));
  const journeys: HomeJourney[] = known.map(({ reading }) => ({
    reading: reading.status,
    participates: reading.status === "ready" && reading.mine.length > 0,
    ready: reading.status === "ready" ? reading.ready.length : 0,
  }));
  const state = homeState({
    viewer: failed !== undefined ? "failed" : viewer === undefined ? "loading" : { identities: viewer.identities.length },
    index: view.status === "failed" ? "failed" : found === undefined ? "loading" : "ready",
    journeys,
    proposals: proposals.view.status === "failed" ? "failed" : waitingProposals === undefined ? "loading" : { waiting: waitingProposals.count },
  });
  return { proposals, failed, view, refresh, found, items, report, known, journeys, state };
}

/** C14, C16: the open proposals waiting on the viewer, with the destination each is for; its own read, so the rest of Home does not wait on it, and rows stay through a failed refresh. */
function ProposalsToReview({ reading }: { reading: ReturnType<typeof useLive<Proposals>> }) {
  const { view, refresh } = reading;
  const found = valueOf(view);
  const rows: PanelRow[] = (found?.items ?? []).map((item) => ({ key: item.id, render: () => <ProposalRow item={item} context={destinationWords(item)} /> }));
  return (
    <>
      {view.status === "failed" || view.status === "behind" ? (
        <p className="callout callout-bad" role="alert" data-testid="home-proposals-failed">
          <Failed what="The proposals waiting on you" verb={found === undefined ? "read" : "refreshed"} error={view.error}>
            <button type="button" className="ghost" onClick={refresh}>
              Try again
            </button>
          </Failed>
        </p>
      ) : null}
      {rows.length === 0 ? null : <RowsPanel title="Proposals to review" testId="home-proposals" rows={rows} folded />}
    </>
  );
}

/** What Home says in place of its groups, for the states that have none. */
function Said({ state }: { state: HomeState }) {
  switch (state) {
    case "loading":
      return <p className="muted home-reading">Reading your journeys...</p>;
    case "unlinked":
      return (
        <Statement title="Cairn cannot match your sign-in to a person yet, so nothing shows as yours." testId="home-unlinked">
          <p>No person or team holds a verified email of yours.</p>
          <Link to={pathTo("me")}>Check who you are &rsaquo;</Link>
        </Statement>
      );
    case "unassigned":
      return (
        <Statement title="No work in an active journey is assigned to you." testId="home-unassigned">
          <span className="row">
            <Link to={indexPath()}>All journeys &rsaquo;</Link>
            <Link to={newJourneyPath()}>Start a journey &rsaquo;</Link>
          </span>
        </Statement>
      );
    case "quiet":
      return (
        <Statement title="Nothing needs you now." testId="home-quiet">
          <p>What is yours is waiting on other work, or already finished.</p>
        </Statement>
      );
    case "failed":
    case "ready":
      return null;
  }
}

export function Home() {
  const { proposals, failed, view, refresh, found, items, report, known, journeys, state } = useHome();
  const readyOnes = known.flatMap((each) => (each.reading.status === "ready" && each.reading.ready.length > 0 ? [{ summary: each.summary, reading: each.reading }] : []));
  const waiting = known.flatMap((each) => (each.reading.status === "ready" && each.reading.ready.length === 0 ? [{ summary: each.summary, reading: each.reading }] : []));
  const reading = known.filter((each) => each.reading.status === "loading").length;
  const line = readyLine(journeys);
  const showTable = state === "ready" || state === "quiet";
  return (
    <section className="stack home rows-page" aria-label="Home" data-testid="home" data-rows-page data-state={state} tabIndex={-1}>
      <h1>Waiting on you</h1>
      {found === undefined ? null : items.map((summary) => <Reader key={summary.id} id={summary.id} report={report} />)}
      {line === undefined ? null : (
        <p className="home-count" data-testid="home-count">
          <span className="mono">{line.rows}</span> ready for you in <span className="mono">{line.journeys}</span> {line.journeys === 1 ? "journey" : "journeys"}
        </p>
      )}
      {failed === undefined ? null : <p className="callout callout-bad"><Failed what="Who you are" error={failed} /></p>}
      {view.status === "failed" ? (
        <p className="callout callout-bad">
          <Failed what="The journeys" error={view.error}>
            <button type="button" className="ghost" onClick={refresh}>
              Try again
            </button>
          </Failed>
        </p>
      ) : null}
      <Said state={state} />
      {showTable ? (
        <ul className="plain-list stack home-list">
          {known.map(({ summary, reading: each }) =>
            each.status === "failed" ? (
              <FailedGroup key={summary.id} summary={summary} message={each.message} />
            ) : each.status === "ready" && each.ready.length > 0 ? (
              <Group key={summary.id} summary={summary} reading={each} />
            ) : null,
          )}
        </ul>
      ) : null}
      {state === "ready" && reading > 0 ? (
        <p className="muted home-reading" data-testid="home-reading">
          Reading {plural(reading, "more journey")}...
        </p>
      ) : null}
      <ProposalsToReview reading={proposals} />
      {showTable ? <NotReady entries={waiting} folded={readyOnes.length > 0} /> : null}
      {showTable ? (
        <Link to={indexPath()} className="home-all" data-testid="home-all-journeys">
          All journeys &rsaquo;
        </Link>
      ) : null}
    </section>
  );
}
