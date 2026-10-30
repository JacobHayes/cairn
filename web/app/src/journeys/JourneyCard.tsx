// C16: the journey card, the inspector's empty state on the views other than the Summary: how
// much is done, who owns it, what needs a look, what is still the viewer's, what is next, the next
// milestone, with the open decisions, the description, the notes and links, and the lineage as
// folds. The Summary (summary/SummaryPage.tsx) is the same journey at full width. Kept current
// (H6): both read the journey's local derivation.
import { STATUS_HOLDS_CURRENT_WORK } from "@cairn/client";

import { useProjected } from "../canvas/hooks.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { AnnotationList } from "../detail/Attachments.tsx";
import type { Ready } from "../detail/model.ts";
import { NodeLink, Section } from "../detail/parts.tsx";
import { summaryModel, type SummaryModel } from "../summary/model.ts";
import { dateInk, inkClass, originWord, whenWords } from "../status/when.ts";
import { Meter } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { yoursOf } from "./figures-model.ts";
import { Flags, LineageBlock, OwnersLine, useLineage } from "./JourneyFacts.tsx";
import { useMineOf } from "./mine.ts";
import { stagesOf } from "./stages.ts";
import { StagesSection } from "./StagesSection.tsx";
import { useMemo } from "react";
import "./journeys.css";
import { Failed } from "../ui/Failed.tsx";
import { plural } from "../ui/words.ts";
import { ownerWords } from "../detail/sections.tsx";
import { useCaller } from "../data/react.ts";
import { Tip } from "../ui/Tip.tsx";
import { Words } from "../ui/Words.tsx";

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
    <Section title="Lineage" testId="card-lineage">
      <LineageBlock ready={ready} lineage={lineage} />
    </Section>
  );
}

/** `You have 3 open items here, 2 ready`, for a participant who arrives cold; nothing when none is open, or the journey holds no current work (B11). */
function Yours({ ready }: { ready: Ready }) {
  const mine = useMineOf(ready.journey.header.id);
  if (mine.status !== "ready" || !STATUS_HOLDS_CURRENT_WORK[ready.journey.header.status]) {
    return null;
  }
  const { open, actionable } = yoursOf(mine.entries);
  if (open === 0) {
    return null;
  }
  return (
    <span className="muted" data-testid="card-yours" data-open={open} data-ready={actionable}>
      You have {plural(open, "open item")} here{actionable === 0 ? "" : `, ${String(actionable)} ready`}
    </span>
  );
}

/** What can move next; on a journey that holds no current work (B11) it is what was left ready or active, as the Summary words it. */
function NextUp({ ready }: { ready: Ready }) {
  const next = valueOf(useProjected(ready, { projection: "next" }));
  const items = (next?.items ?? []).slice(0, NEXT_UP_SHOWN);
  if (items.length === 0) {
    return null;
  }
  const heading = STATUS_HOLDS_CURRENT_WORK[ready.journey.header.status] ? "Next up" : "Ready or active";
  return (
    <section className="stack" aria-label={heading} data-testid="card-next-up">
      <span className="muted">{heading}</span>
      <ul className="detail-list">
        {items.map((row) => (
          <li key={row.key} data-node={row.key}>
            <NodeLink view={ready} node={row.key} />
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
      <span className="muted">{STATUS_HOLDS_CURRENT_WORK[ready.journey.header.status] ? "Next milestone" : "Planned milestone"}</span>
      <div>
        <div className="detail-clamp">
          <NodeLink view={ready} node={next.key} />
        </div>
        <Tip tip={`This date is ${originWord(next.origin)}`} tap>
          <span className={inkClass(dateInk(next.date, ready.derived.today, next.late)) ?? "muted"}><Words text={whenWords(next.date, next.origin, ready.derived.today, next.late)} /></span>
        </Tip>
      </div>
    </section>
  );
}

function Folds({ ready, model }: { ready: Ready; model: SummaryModel }) {
  const { header } = ready.journey;
  const { caller } = useCaller();
  const annotations = (ready.journey.graph.state?.annotations ?? []).filter((annotation) => annotation.body.node == null);
  return (
    <div className="detail-folds" data-testid="card-folds">
      {model.openDecisions.length === 0 ? null : (
        <Section title="Open decisions" summary={String(model.openDecisions.length)} testId="card-open-decisions">
          <ul className="detail-list">
            {model.openDecisions.map((each) => (
              <li key={each.key} data-node={each.key}>
                <NodeLink view={ready} node={each.key} />{" "}
                <span className="muted">{ownerWords(ready, each.owners, caller?.identities)}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}
      <StagesSection stages={stagesOf(ready)} />
      <Section title="Description" testId="card-description">
        {header.description == null ? <span className="muted">No description.</span> : <Markdown text={header.description} data-testid="overview-description" />}
      </Section>
      <Section title="Notes and links" summary={String(annotations.length)} testId="card-notes">
        <AnnotationList view={ready} node={null} annotations={annotations} bare />
      </Section>
      <LineageFold ready={ready} />
    </div>
  );
}

/** The journey card: the inspector's empty state on every view but the Summary, which is the journey's front page. */
export function JourneyCard({ ready }: { ready: Ready }) {
  const summaryRead = useProjected(ready, { projection: "status_summary" });
  const summary = valueOf(summaryRead);
  const summaryError = errorWords(summaryRead);
  const model = useMemo(() => (summary === undefined ? undefined : summaryModel(ready, summary)), [ready, summary]);
  const done = model === undefined ? undefined : model.inScope - model.remaining;
  return (
    <section className="journey-card detail-panel panel stack" aria-label="Journey" data-testid="journey-card">
      <div className="stack detail-lead">
        <span className="muted">Journey</span>
        {model === undefined || done === undefined ? null : (
          <>
            <span data-testid="card-progress" data-done={done} data-in-scope={model.inScope} data-remaining={model.remaining}>
              {done} of {model.inScope} in scope done{model.remaining === 0 ? "" : `, ${String(model.remaining)} ${STATUS_HOLDS_CURRENT_WORK[ready.journey.header.status] ? "to go" : "unfinished"}`}
            </span>
            <Meter done={done} of={model.inScope} />
          </>
        )}
      </div>
      {summaryError === undefined ? null : <p className="callout callout-bad"><Failed what="The summary" error={summaryError} /></p>}
      {model === undefined || done === undefined ? (
        summaryError === undefined ? <p className="muted">Summing up the journey...</p> : null
      ) : (
        <>
          <OwnersLine ready={ready} />
          <Yours ready={ready} />
          <Flags ready={ready} model={model} />
          <NextUp ready={ready} />
          <NextMilestone ready={ready} model={model} />
          <Folds ready={ready} model={model} />
        </>
      )}
    </section>
  );
}
