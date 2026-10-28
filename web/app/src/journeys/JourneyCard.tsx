// C16: the journey card, the inspector's empty state on the views other than the Summary: how
// much is done, who owns it, what needs a look, what is still the viewer's, what is next, the next
// milestone, with the open decisions, the description, the notes and links, and the lineage as
// folds. The Summary (summary/SummaryPage.tsx) is the same journey at full width. Kept current
// (H6): both read the journey's local derivation.
import { STATUS_HOLDS_CURRENT_WORK } from "@cairn/client";

import { usePoint } from "../canvas/pointed.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { useProjected } from "../canvas/hooks.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { AnnotationList } from "../detail/Attachments.tsx";
import type { Ready } from "../detail/model.ts";
import { NodeLink, Section } from "../detail/parts.tsx";
import { summaryModel, type SummaryModel } from "../summary/model.ts";
import { dateInk, inkClass, originWord, whenWords } from "../status/when.ts";
import { statusTone, statusWord } from "../status/words.ts";
import { Badge, Meter } from "../ui/kit.tsx";
import { progressWords } from "../canvas/words.ts";
import { Markdown } from "../ui/markdown.tsx";
import { yoursOf } from "./figures-model.ts";
import { Flags, LineageBlock, OwnersLine, useLineage } from "./JourneyFacts.tsx";
import { useMineOf } from "./mine.ts";
import { stagesOf } from "./stages.ts";
import { useEffect, useMemo } from "react";
import "./journeys.css";
import { Failed } from "../ui/Failed.tsx";
import { plural } from "../ui/words.ts";
import { ownerWords } from "../detail/sections.tsx";
import { useCaller } from "../data/react.ts";
import { Tip } from "../ui/Tip.tsx";

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

/** `You have 3 open items here, 2 ready`, for a participant who arrives cold (2.4); nothing when none is open, or the journey holds no current work (B11). */
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
          <span className={inkClass(dateInk(next.date, ready.derived.today)) ?? "muted"}>{whenWords(next.date, next.origin, ready.derived.today)}</span>
        </Tip>
      </div>
    </section>
  );
}

/** One stage as a row, the whole row a target: choosing it selects the stage and frames it on the canvas; pointing at it outlines the stage there. */
function StagesFold({ ready, framing }: { ready: Ready; framing: boolean }) {
  const stages = stagesOf(ready);
  const actions = useFrameActions();
  const point = usePoint();
  // Choosing a row replaces the card: the list's own pointing ends with it.
  useEffect(() => () => { point(undefined); }, [point]);
  // Where the stage is framed on a canvas under a sheet, it would be framed in no room at all: the sheet drops to its peek, where a card picked on the canvas opens it too.
  const lower = () => { if (framing) { actions?.snap("peek"); } };
  if (stages.length === 0) {
    return null;
  }
  return (
    <Section title="Stages" summary={String(stages.length)} testId="card-stages">
      <ul className="plain-list stage-rows">
        {stages.map((stage) => (
          <li
            key={stage.key}
            className="stage-row"
            data-testid="stage-row"
            data-node={stage.key}
            data-state={stage.displayState}
            onPointerEnter={() => { point(stage.key); }}
            onPointerLeave={() => { point(undefined); }}
            onFocus={() => { point(stage.key); }}
            onBlur={() => { point(undefined); }}
          >
            <span className="stage-row-head">
              <NodeLink view={ready} node={stage.key} state={{ reveal: { key: stage.key, how: "frame" } }} onClick={lower} />
              <Badge tone={statusTone(stage.displayState)}>{statusWord(stage.displayState, stage.kind)}</Badge>
            </span>
            <span className="stage-row-foot muted">
              <Meter done={stage.done} of={stage.total} />
              <span>{progressWords(stage.done, stage.total)}</span>
              {stage.due === undefined ? null : <span className={["stage-row-due", stage.dueInk].filter(Boolean).join(" ")}>{stage.due}</span>}
            </span>
          </li>
        ))}
      </ul>
    </Section>
  );
}

function Folds({ ready, model, selected, framing }: { ready: Ready; model: SummaryModel; selected: string | undefined; framing: boolean }) {
  const { header } = ready.journey;
  const { caller } = useCaller();
  const annotations = (ready.journey.graph.state?.annotations ?? []).filter((annotation) => annotation.body.node == null);
  return (
    <div className="detail-folds" data-testid="card-folds">
      {model.openDecisions.length === 0 ? null : (
        <Section title="Open decisions" summary={String(model.openDecisions.length)} testId="card-open-decisions">
          <ul className="detail-list">
            {model.openDecisions.map((each) => (
              <li key={each.key} data-node={each.key} data-selected={each.key === selected}>
                <NodeLink view={ready} node={each.key} />{" "}
                <span className="muted">{ownerWords(ready, each.owners, caller?.identities)}</span>
              </li>
            ))}
          </ul>
        </Section>
      )}
      <StagesFold ready={ready} framing={framing} />
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
/** `framing`: the view is the graph, where choosing a stage frames it. */
export function JourneyCard({ ready, selected, framing = false }: { ready: Ready; selected?: string | undefined; framing?: boolean }) {
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
          <Folds ready={ready} model={model} selected={selected} framing={framing} />
        </>
      )}
    </section>
  );
}
