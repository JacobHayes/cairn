// C16, C18: the journey card, the inspector's empty state on NEXT and PLAN, and the Summary
// page, which is the same component at full width (4.7). An observer gets the status summary's
// figures with no click: how much is done, what needs a look, what is next, the next milestone.
// Open decisions, the description, the notes and links, and the lineage are folds. On the
// Summary page every fold is open and each list is whole: the status summary's parts, with
// the print view (C18). Kept current (H6): both read the journey's local derivation.
// What is the viewer's is one muted line under the progress.
import { Link } from "react-router";

import { DEFAULT_LIST, listPath, type ListFlag } from "../acting/address.ts";
import { DetailLink } from "../acting/Parts.tsx";
import { useProjected } from "../canvas/hooks.ts";
import { useLive } from "../data/react.ts";
import { indexKey, journeyIndex, routeIndex } from "../data/reads.ts";
import { AnnotationList } from "../detail/Attachments.tsx";
import type { Ready } from "../detail/model.ts";
import { NodeLink, Section } from "../detail/parts.tsx";
import { summaryModel, type SummaryModel } from "../summary/model.ts";
import { StatusSummaryView } from "../summary/StatusSummary.tsx";
import { dateWords, dayOf } from "../timeline/model.ts";
import { Markdown } from "../ui/markdown.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { pagePath, summaryPath, type JourneyPage } from "./address.ts";
import { UpgradeMark } from "./JourneyIndex.tsx";
import { useMineOf } from "./mine.ts";
import { routeDetailPath } from "../routes/address.ts";
import { useMemo } from "react";
import "./journeys.css";

/** The most items "next up" lists on the card; the Summary page lists every one. */
const NEXT_UP_SHOWN = 3;

/**
 * C16: the lineage fold: the route and version it is named by (the journey's row in the host's
 * index gives the host's "upgrade available"), and the day it started.
 */
function LineageFold({ ready, full }: { ready: Ready; full: boolean }) {
  const { header } = ready.journey;
  const lineage = header.lineage ?? undefined;
  const query = lineage === undefined ? undefined : { route: lineage.route, version: lineage.version };
  const { view } = useLive(indexKey(query), journeyIndex(query));
  const routes = useLive("routes", routeIndex).view;
  const summary = view.status === "ready" ? view.value.find((item) => item.id === header.id) : undefined;
  const route = lineage === undefined || routes.status !== "ready" ? undefined : routes.value.find((each) => each.header.id === lineage.route);
  const name = lineage === undefined ? undefined : (route?.header.name ?? lineage.route);
  return (
    <Section title="Lineage" summary={lineage === undefined ? "no route" : `${name ?? ""} v${String(lineage.version)}`} open={full} testId="card-lineage">
      {lineage === undefined ? (
        <span className="muted small" data-testid="overview-lineage">Started empty: no route, so no upgrades.</span>
      ) : (
        <span className="row" data-testid="overview-lineage">
          <Link to={routeDetailPath(lineage.route)}>
            {name}, version {lineage.version}
          </Link>
          <Link to={`/routes/${lineage.route}?version=${String(lineage.version)}`} data-testid="lineage">
            See its graph
          </Link>
          {summary === undefined ? null : <UpgradeMark summary={summary} />}
        </span>
      )}
      <span className="muted small">Started {dateWords(header.created_on, ready.derived.today)}</span>
    </Section>
  );
}

/** `You own 3 nodes here, 1 ready`, for a participant who arrives cold (2.4); nothing when nothing is theirs. */
function Yours({ ready }: { ready: Ready }) {
  const mine = useMineOf(ready.journey.header.id);
  if (mine.status !== "ready" || mine.entries.length === 0) {
    return null;
  }
  const frontier = new Set(ready.derived.acting_frontier);
  const actionable = mine.entries.filter((entry) => frontier.has(entry.node)).length;
  return (
    <span className="muted small" data-testid="card-yours" data-owned={mine.entries.length} data-ready={actionable}>
      You own {mine.entries.length} {mine.entries.length === 1 ? "node" : "nodes"} here{actionable === 0 ? "" : `, ${String(actionable)} ready`}
    </span>
  );
}

/** What needs a look, as one short sentence of links into the plan list: only what is not zero. */
function Flags({ ready, model }: { ready: Ready; model: SummaryModel }) {
  const journey = ready.journey.header.id;
  const all: { count: number; words: string; flag: ListFlag }[] = [
    { count: model.overdue.length, words: "overdue", flag: "overdue" },
    { count: model.stale.length, words: "stale", flag: "stale" },
    { count: model.shortfalls.length, words: "short of days", flag: "shortfall" },
  ];
  const flags = all.filter((each) => each.count > 0);
  if (flags.length === 0) {
    return null;
  }
  return (
    <span data-testid="card-flags">
      {flags.map((each, at) => (
        <span key={each.flag}>
          {at === 0 ? "" : ", "}
          <Link to={listPath(journey, { ...DEFAULT_LIST, flags: [each.flag] })} data-testid={`card-flag-${each.flag}`}>
            {each.count} {each.words}
          </Link>
        </span>
      ))}
    </span>
  );
}

function NextUp({ ready, all }: { ready: Ready; all: boolean }) {
  const next = useProjected(ready, { projection: "next" });
  const items = (next.value?.items ?? []).slice(0, all ? undefined : NEXT_UP_SHOWN);
  if (items.length === 0) {
    return null;
  }
  return (
    <section className="stack" aria-label="Next up" data-testid="card-next-up">
      <span className="muted small">Next up</span>
      <ul className="detail-list">
        {items.map((row) => (
          <li key={row.key} data-node={row.key}>
            <DetailLink view={ready} node={row.key} />
          </li>
        ))}
      </ul>
    </section>
  );
}

function NextMilestone({ ready, model }: { ready: Ready; model: SummaryModel }) {
  const next = model.upcoming[0];
  if (next === undefined) {
    return null;
  }
  const days = dayOf(next.date) - dayOf(ready.derived.today);
  return (
    <section className="stack" aria-label="Next milestone" data-testid="card-next-milestone" data-node={next.key}>
      <span className="muted small">Next milestone</span>
      <span>
        <NodeLink view={ready} node={next.key} /> <span className="muted small">{dateWords(next.date, ready.derived.today)}, {days < 0 ? `${String(-days)} days late` : days === 0 ? "today" : `in ${String(days)} days`}</span>
      </span>
    </section>
  );
}

function Folds({ ready, model, full, selected }: { ready: Ready; model: SummaryModel; full: boolean; selected: string | undefined }) {
  const { header } = ready.journey;
  const annotations = (ready.journey.graph.state?.annotations ?? []).filter((annotation) => annotation.body.node == null);
  return (
    <div className="stack" data-testid="card-folds">
      {full || model.openDecisions.length === 0 ? null : (
        <Section title="Open decisions" summary={String(model.openDecisions.length)} testId="card-open-decisions">
          <ul className="detail-list">
            {model.openDecisions.map((each) => (
              <li key={each.key} data-node={each.key} data-selected={each.key === selected}>
                <NodeLink view={ready} node={each.key} />{" "}
                <span className="muted small">{each.owners.length === 0 ? "unassigned" : each.owners.join(", ")}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}
      <Section title="Description" open={full} testId="card-description">
        {header.description == null ? <span className="muted small">No description.</span> : <Markdown text={header.description} data-testid="overview-description" />}
      </Section>
      <Section title="Notes and links" summary={String(annotations.length)} open={full} testId="card-notes">
        <AnnotationList view={ready} node={null} annotations={annotations} bare />
      </Section>
      <LineageFold ready={ready} full={full} />
    </div>
  );
}

/**
 * The journey card: the inspector's empty state (`full` off), or the Summary page, the same
 * card at full width with every fold open and each list whole (`full`).
 */
export function JourneyCard({ ready, page, selected, full = false }: { ready: Ready; page: JourneyPage | "summary"; selected?: string | undefined; full?: boolean }) {
  const journey = ready.journey.header.id;
  const summary = useProjected(ready, { projection: "status_summary" });
  const model = useMemo(() => (summary.value === undefined ? undefined : summaryModel(ready, summary.value)), [ready, summary.value]);
  const { header } = ready.journey;
  const done = model === undefined ? undefined : model.inScope - model.remaining;
  return (
    <section
      className={full ? "journey-card journey-card-full stack" : "journey-card detail-panel panel stack"}
      aria-label={full ? "Summary" : "Journey"}
      data-testid="journey-card"
      data-full={full}
    >
      <div className="row">
        <span className="muted small">{full ? `Summary, as of ${dateWords(ready.derived.today, ready.derived.today)}` : "Journey"}</span>
        {full ? <Badge data-testid="card-status" data-status={header.status}>{header.status}</Badge> : null}
        <span className="spacer" />
        {full ? (
          <>
            <Button className="no-print" onClick={() => { globalThis.print(); }}>
              Print
            </Button>
            <Link to={pagePath(journey, "next", "list")} className="no-print" data-testid="summary-close" aria-label="Close the summary">
              Close
            </Link>
          </>
        ) : (
          <Link to={summaryPath(journey)} className="icon-button" data-testid="card-expand" aria-label="Open the summary page" title="Open the summary page">
            ⤢
          </Link>
        )}
      </div>
      {summary.error === undefined ? null : <p className="callout callout-bad">The summary could not be read: {summary.error}</p>}
      {model === undefined || done === undefined ? (
        summary.error === undefined ? <p className="muted small">Summing up the journey...</p> : null
      ) : (
        <>
          <span data-testid="card-progress" data-done={done} data-in-scope={model.inScope}>
            {done} of {model.inScope} in scope done
          </span>
          <progress value={done} max={Math.max(model.inScope, 1)} aria-label="Progress" />
          <Yours ready={ready} />
          <Flags ready={ready} model={model} />
          {full ? <NextUp ready={ready} all /> : page === "next" ? null : <NextUp ready={ready} all={false} />}
          {full ? <StatusSummaryView ready={ready} model={model} selected={selected} /> : <NextMilestone ready={ready} model={model} />}
          <Folds ready={ready} model={model} full={full} selected={selected} />
        </>
      )}
    </section>
  );
}
