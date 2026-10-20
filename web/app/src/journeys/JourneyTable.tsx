// The journeys as a table (C16), the journey index's: each journey with where it
// came from, what needs a look, what is the viewer's, how far along it is and its next milestone.
// The index holds only summaries, so each row reads its journey in the tab (reading.ts) and says
// "Reading..." or why it could not until it has. With `mine` only the journeys where the viewer
// holds something are kept (E4). "Upgrade available" is the host's field on each journey, never
// worked out here. Kept current (H6).
import { Failed } from "../ui/Failed.tsx";
import { useMemo, useState, type ReactNode } from "react";
import { Link } from "react-router";

import { useRetryJourney } from "../data/react.ts";
import type { JourneySummary, RouteSummary } from "../data/reads.ts";
import { Badge, Meter } from "../ui/kit.tsx";
import { routePath } from "../routes/address.ts";
import { forYouOf, forYouWords, milestoneOf, progressOf, troubleOf, troubleWords, upgradeWords, type ForYou, type Trouble } from "./figures-model.ts";
import { journeyPath } from "./address.ts";
import { useMineCounts, useReportMine } from "./mine.ts";
import { useJourneyReading, type JourneyReading } from "./reading.ts";
import { useSeen } from "./useSeen.ts";
import { listPath } from "./view.ts";

/** A journey's lineage: its route, by name when the route index has it, and version, or that it started empty. */
function Lineage({ summary, routes }: { summary: JourneySummary; routes: readonly RouteSummary[] }) {
  const lineage = summary.lineage;
  if (lineage == null) {
    return <span data-testid="row-lineage-none">Started empty</span>;
  }
  const route = routes.find((each) => each.header.id === lineage.route);
  return (
    <Link to={routePath(lineage.route, { version: lineage.version })} data-testid="row-lineage">
      {route?.header.name ?? lineage.route}, version {lineage.version}
    </Link>
  );
}

/** A journey's name, its status when it is not active, and where it came from (with its upgrade): the cell that needs nothing derived. */
export function JourneyCell({ summary, routes }: { summary: JourneySummary; routes: readonly RouteSummary[] }) {
  const upgrade = upgradeWords(summary);
  return (
    <td data-column="journey">
      <span className="cell-first">
        <Link to={journeyPath({ journey: summary.id })}>
          {summary.name}
        </Link>
        {summary.status === "active" ? null : <Badge>{summary.status}</Badge>}
      </span>
      <span className="cell-second">
        <span className="cell-lineage">
          <Lineage summary={summary} routes={routes} />
        </span>
        {upgrade === undefined ? null : (
          <>
            <span className="cell-lineage">{" · "}</span>
            <span className="flag-warn" data-testid="upgrade" data-status="available">
              {upgrade}
            </span>
          </>
        )}
      </span>
    </td>
  );
}

/** What needs a look: the worst thing, linking to where it is, and how much else there is. */
function TroubleCell({ journey, troubles }: { journey: string; troubles: Trouble[] }) {
  const [first, ...rest] = troubles;
  if (first === undefined) {
    return (
      <td data-column="trouble" data-testid="row-trouble" data-kind="none">
        <span className="muted">None</span>
      </td>
    );
  }
  const tone = first.tone === undefined ? undefined : `flag-${first.tone}`;
  return (
    <td data-column="trouble" data-testid="row-trouble" data-kind={first.kind}>
      <Link to={first.kind === "stalled" ? journeyPath({ journey }) : listPath(journey, { flags: [first.kind] })} {...(tone === undefined ? {} : { className: tone })}>
        {troubleWords(first)}
      </Link>
      {rest.length === 0 ? null : (
        <span className="cell-second" title={rest.map(troubleWords).join(", ")}>
          +{rest.length} more
        </span>
      )}
    </td>
  );
}

/** What is the viewer's here, linking to the journey's list of their work when there is some. */
function ForYouCell({ journey, forYou }: { journey: string; forYou: ForYou }) {
  const words = forYouWords(forYou);
  return (
    <td data-column="for-you" data-testid="row-mine" data-for-you={forYou.kind}>
      {forYou.kind === "ready" || forYou.kind === "waiting" ? <Link to={listPath(journey, { mine: true })}>{words}</Link> : <span className="muted">{words}</span>}
    </td>
  );
}

/** How much is left of what is in scope, in mono, over a quiet meter. */
function ProgressCell({ left, inScope }: { left: number; inScope: number }) {
  const done = inScope - left;
  return (
    <td className="num" data-column="progress" data-testid="row-progress" data-done={done} data-in-scope={inScope}>
      {inScope === 0 ? (
        <span className="muted">Nothing planned</span>
      ) : (
        <>
          <span className="cell-first mono">{left === 0 ? "Nothing left" : `${String(left)} left of ${String(inScope)}`}</span>
          <Meter done={done} of={inScope} label={`${String(done)} of ${String(inScope)} finished or skipped`} />
        </>
      )}
    </td>
  );
}

/** The next milestone and its date, in the ink a soon or late date takes. */
function MilestoneCell({ reading }: { reading: Extract<JourneyReading, { status: "ready" }> }) {
  const next = reading.model === undefined ? undefined : milestoneOf(reading.model, reading.view.derived.today);
  return (
    <td data-column="milestone" data-testid="row-milestone">
      {next === undefined ? (
        <span className="cell-first muted">No dated milestones</span>
      ) : (
        <>
          <span className="cell-first">{next.title}</span>
          <span className={next.tone === undefined ? "cell-second mono" : `cell-second mono flag-${next.tone}`}>{next.when}</span>
        </>
      )}
    </td>
  );
}

/** A row's four derived columns, as one cell that says they are being read, or why they could not be. */
function ReadingCell({ reading, id }: { reading: Exclude<JourneyReading, { status: "ready" }>; id: string }) {
  const retry = useRetryJourney();
  if (reading.status === "failed") {
    return (
      <td colSpan={4} className="cell-reading" data-column="reading" data-testid="row-failed">
        <Failed what="This journey" error={reading.message}>
          <button type="button" className="ghost" onClick={() => { retry(id); }}>
            Try again
          </button>
        </Failed>
      </td>
    );
  }
  return (
    <td colSpan={4} className="cell-reading muted" data-column="reading" data-testid="row-reading">
      Reading...
    </td>
  );
}

/** A row's attributes, kept whether or not its journey is being read. */
function attrsOf(summary: JourneySummary) {
  return { "data-testid": "journey-row", "data-journey": summary.id, "data-status": summary.status, "data-revision": summary.revision } as const;
}

type RowProps = { summary: JourneySummary; routes: readonly RouteSummary[]; mine: boolean; report: (id: string, count: number | undefined) => void; rowRef: (row: HTMLTableRowElement | null) => void };

/** A journey's row with its journey read: the derived columns, or why there are none. */
function ReadRow({ summary, routes, mine, report, rowRef }: RowProps) {
  const reading = useJourneyReading(summary.id, true);
  useReportMine(summary.id, reading.status === "ready" ? reading.mine.length : reading.status === "gone" ? 0 : undefined, report);
  if (reading.status === "gone" || (mine && reading.status === "ready" && reading.mine.length === 0)) {
    return null;
  }
  if (reading.status !== "ready") {
    return (
      <tr ref={rowRef} {...attrsOf(summary)}>
        <JourneyCell summary={summary} routes={routes} />
        <ReadingCell reading={reading} id={summary.id} />
      </tr>
    );
  }
  const { model } = reading;
  const troubles = model === undefined ? [] : troubleOf(model, reading.view.derived.stalled != null);
  const forYou = forYouOf(reading.view, summary.status, reading.mine, reading.ready.length);
  return (
    <tr ref={rowRef} {...attrsOf(summary)}>
      <JourneyCell summary={summary} routes={routes} />
      <TroubleCell journey={summary.id} troubles={troubles} />
      <ForYouCell journey={summary.id} forYou={forYou} />
      <ProgressCell left={model?.remaining ?? 0} inScope={model === undefined ? 0 : progressOf(model).inScope} />
      <MilestoneCell reading={reading} />
    </tr>
  );
}

/**
 * A journey's row. With `mine` every row stays read (its count decides which rows exist, and a live
 * change can add one); otherwise a row is read only while it is near the view, so a long index
 * derives the journeys on screen and lets the rest go (the journey manager parks a bounded few).
 */
function JourneyRow(props: Omit<RowProps, "rowRef">) {
  const [row, setRow] = useState<HTMLTableRowElement | null>(null);
  const seen = useSeen(row, props.mine);
  if (seen) {
    return <ReadRow {...props} rowRef={setRow} />;
  }
  return (
    <tr ref={setRow} {...attrsOf(props.summary)}>
      <JourneyCell summary={props.summary} routes={props.routes} />
      <td colSpan={4} className="cell-reading muted" data-column="reading" data-testid="row-reading">
        Reading...
      </td>
    </tr>
  );
}

/**
 * The journeys as a table, by name: every one of `items`, or with `mine` only those where the caller
 * holds something (E4). `none` is said in its place when no journey is kept; with `mine` every
 * candidate stays mounted (kept current) though its row may be empty.
 */
export function JourneyTable({ items, routes, mine, none }: { items: JourneySummary[]; routes: RouteSummary[]; mine: boolean; none: ReactNode }) {
  const { report, none: nobody } = useMineCounts(mine ? items : []);
  const sorted = useMemo(() => [...items].sort((left, right) => left.name.localeCompare(right.name)), [items]);
  if (items.length === 0) {
    return none;
  }
  return (
    <>
      {mine && nobody ? none : null}
      <div className="index-table" data-testid="journey-table" hidden={mine && nobody}>
        <table className="data">
          <thead>
            <tr>
              <th scope="col">Journey</th>
              <th scope="col">Needs a look</th>
              <th scope="col">For you</th>
              <th scope="col" className="num">
                Progress
              </th>
              <th scope="col">Next milestone</th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((summary) => (
              <JourneyRow key={summary.id} summary={summary} routes={routes} mine={mine} report={report} />
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}
