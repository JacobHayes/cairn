// C16: the journey card, the inspector's empty state on the views other than the Summary: how
// much is done, what needs a look, what is still the viewer's, what is next, the next
// milestone, with the open decisions, the description, the notes and links, and the lineage as
// folds. The Summary (summary/SummaryPage.tsx) is the same journey at full width. Kept current
// (H6): both read the journey's local derivation.
import { DetailLink } from "../acting/Parts.tsx";
import { useProjected } from "../canvas/hooks.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { AnnotationList } from "../detail/Attachments.tsx";
import type { Ready } from "../detail/model.ts";
import { NodeLink, Section } from "../detail/parts.tsx";
import { summaryModel, type SummaryModel } from "../summary/model.ts";
import { dateAway } from "../timeline/model.ts";
import { Meter } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { yoursOf } from "./figures-model.ts";
import { Flags, LineageBlock, useLineage } from "./JourneyFacts.tsx";
import { useMineOf } from "./mine.ts";
import { useMemo } from "react";
import "./journeys.css";
import { ReadFailed } from "../ui/ReadFailed.tsx";

/** The most items "next up" lists on the card; the Summary lists the whole frontier. */
const NEXT_UP_SHOWN = 3;

/** C16: the lineage fold: the route and version the journey is named by, and the day it started. */
function LineageFold({ ready }: { ready: Ready }) {
  const lineage = useLineage(ready);
  // A journey that did not come from a route has no lineage to show.
  if (lineage === undefined) {
    return null;
  }
  return (
    <Section title="Lineage" open={lineage.latest !== undefined} testId="card-lineage">
      <LineageBlock ready={ready} lineage={lineage} />
    </Section>
  );
}

/** `You have 3 open items here, 2 ready`, for a participant who arrives cold (2.4); nothing when none is open. */
function Yours({ ready }: { ready: Ready }) {
  const mine = useMineOf(ready.journey.header.id);
  if (mine.status !== "ready") {
    return null;
  }
  const { open, actionable } = yoursOf(ready, mine.entries);
  if (open === 0) {
    return null;
  }
  return (
    <span className="muted small" data-testid="card-yours" data-open={open} data-ready={actionable}>
      You have {open} open {open === 1 ? "item" : "items"} here{actionable === 0 ? "" : `, ${String(actionable)} ready`}
    </span>
  );
}

function NextUp({ ready }: { ready: Ready }) {
  const next = valueOf(useProjected(ready, { projection: "next" }));
  const items = (next?.items ?? []).slice(0, NEXT_UP_SHOWN);
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
  return (
    <section className="stack" aria-label="Next milestone" data-testid="card-next-milestone" data-node={next.key}>
      <span className="muted small">Next milestone</span>
      <span>
        <NodeLink view={ready} node={next.key} /> <span className="muted small">{dateAway(next.date, ready.derived.today)}</span>
      </span>
    </section>
  );
}

function Folds({ ready, model, selected }: { ready: Ready; model: SummaryModel; selected: string | undefined }) {
  const { header } = ready.journey;
  const annotations = (ready.journey.graph.state?.annotations ?? []).filter((annotation) => annotation.body.node == null);
  return (
    <div className="detail-folds" data-testid="card-folds">
      {model.openDecisions.length === 0 ? null : (
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
      <Section title="Description" testId="card-description">
        {header.description == null ? <span className="muted small">No description.</span> : <Markdown text={header.description} data-testid="overview-description" />}
      </Section>
      <Section title="Notes and links" summary={String(annotations.length)} testId="card-notes">
        <AnnotationList view={ready} node={null} annotations={annotations} bare />
      </Section>
      <LineageFold ready={ready} />
    </div>
  );
}

/** The journey card: the inspector's empty state on every view but the Summary, which is the journey's front page. */
export function JourneyCard({ ready, selected }: { ready: Ready; selected?: string | undefined }) {
  const summaryRead = useProjected(ready, { projection: "status_summary" });
  const summary = valueOf(summaryRead);
  const summaryError = errorWords(summaryRead);
  const model = useMemo(() => (summary === undefined ? undefined : summaryModel(ready, summary)), [ready, summary]);
  const done = model === undefined ? undefined : model.inScope - model.remaining;
  return (
    <section className="journey-card detail-panel panel stack" aria-label="Journey" data-testid="journey-card">
      <div className="stack detail-lead">
        <span className="muted small">Journey</span>
        {model === undefined || done === undefined ? null : (
          <>
            <span data-testid="card-progress" data-done={done} data-in-scope={model.inScope} data-remaining={model.remaining}>
              {done} of {model.inScope} in scope done{model.remaining === 0 ? "" : `, ${String(model.remaining)} to go`}
            </span>
            <Meter done={done} of={model.inScope} />
          </>
        )}
      </div>
      {summaryError === undefined ? null : <p className="callout callout-bad"><ReadFailed what="The summary" error={summaryError} /></p>}
      {model === undefined || done === undefined ? (
        summaryError === undefined ? <p className="muted small">Summing up the journey...</p> : null
      ) : (
        <>
          <Yours ready={ready} />
          <Flags ready={ready} model={model} />
          <NextUp ready={ready} />
          <NextMilestone ready={ready} model={model} />
          <Folds ready={ready} model={model} selected={selected} />
        </>
      )}
    </section>
  );
}
