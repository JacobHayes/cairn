// C8, F7: a node's dates, each told apart as pin, actual, or derived, each with the chain that
// produced it (conditional links labeled) and the pins and rules to edit to change it; slack;
// a shortfall with its chain and the moves that resolve it (F5, F6). Everything comes from
// the local derive. What editing a pin looks like is the caller's (`pinEditor`), since it
// routes through a feeding decision when there is one (E3).
import type { ReactNode } from "react";

import { dateLinkSentence } from "../canvas/dates.ts";
import { dateWords, originWord } from "../status/when.ts";
import { ChainView } from "./ChainView.tsx";
import { foldKey } from "./folds.ts";
import type { Bound } from "./explain.ts";
import { nodeOf, titleOf, type Mutation, type NodeDetail, type Ready } from "./model.ts";
import { LinkedDateRow, NodeLink, Section } from "./parts.tsx";
import { ShortfallView } from "./ShortfallView.tsx";

/** F7: where a bound comes from. */
export type BoundOrigin = "pin" | "actual" | "derived";

/**
 * F7: a bound is a pin or an actual when nothing lies between it and a date fixed on the
 * node itself; otherwise it is derived through its chain (from a pin, an actual, an answer,
 * or today elsewhere).
 */
export function boundOrigin(bound: Bound, node: string): BoundOrigin {
  const [fixed] = bound.chain.fixed ?? [];
  const onNode = fixed !== undefined && typeof fixed.instant === "object" && "node" in fixed.instant && fixed.instant.node.node === node;
  if (bound.chain.constraints.length > 0 || !onNode) {
    return "derived";
  }
  return fixed.fixed_by === "actual" ? "actual" : fixed.fixed_by === "today" ? "derived" : "pin";
}

function BoundRow({ view, node, label, bound }: { view: Ready; node: string; label: string; bound: Bound | null | undefined }) {
  if (bound == null) {
    return (
      <div className="date-row" data-testid="date" data-label={label} data-origin="none">
        <span className="date-label">{label}</span> <span className="muted small">no date reaches it</span>
      </div>
    );
  }
  const origin = boundOrigin(bound, node);
  return (
    <div className="date-row" data-testid="date" data-label={label} data-origin={origin}>
      <span className="date-label">{label}</span> <strong>{dateWords(bound.date, view.derived.today)}</strong> <span className="muted small">{originWord(origin)}</span>
      <details>
        <summary className="muted small">Why this date</summary>
        <ChainView view={view} chain={bound.chain} />
      </details>
    </div>
  );
}

function Actuals({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { started_on: started, finished_on: finished } = detail.record;
  if (started == null && finished == null) {
    return null;
  }
  return (
    <div className="row" data-testid="actuals">
      {started == null ? null : <span>Started <strong>{dateWords(started, view.derived.today)}</strong></span>}
      {finished == null ? null : <span>Finished <strong>{dateWords(finished, view.derived.today)}</strong></span>}
    </div>
  );
}

/**
 * C1, C8: the node's date relationships in both directions (its due-by and not-before rules, its
 * stage's close, an opening that holds dates only, a decision that sets a milestone's date), each
 * as the sentence the canvas says on hover, with the other node as a link. This is where every date
 * line the canvas draws for the node is read and followed, with the keyboard or by touch.
 */
export function LinkedDates({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const key = detail.node.key;
  const { links = [] } = detail.derived.dates;
  if (links.length === 0) {
    return null;
  }
  const words = (link: (typeof links)[number]) => dateLinkSentence(link, titleOf(view, link.node), titleOf(view, link.source), { decision: nodeOf(view, link.node)?.kind === "decision" }).join("");
  return (
    <Section title="Linked dates" summary={String(links.length)} testId="linked-dates" fold={foldKey(detail.node.kind, "linked-dates")}>
      <ul className="stack linked-dates">
        {links.map((link) => {
          const other = link.node === key ? link.source : link.node;
          return <LinkedDateRow key={`${link.node}|${link.source}|${link.kind}|${"rule" in link ? link.rule : ""}`} link={other} node={<NodeLink view={view} node={other} />} words={words(link)} />;
        })}
      </ul>
    </Section>
  );
}

export function DatesSection({
  view,
  detail,
  pinEditor,
  onMove,
  open = true,
  fold,
}: {
  view: Ready;
  detail: NodeDetail;
  pinEditor?: ReactNode;
  onMove?: ((move: Mutation) => void) | undefined;
  /** Whether it starts open, and the key the viewer's choice is remembered under (folds.ts). */
  open?: boolean;
  fold?: string;
}) {
  const { dates } = detail.derived;
  const key = detail.node.key;
  return (
    <Section title="Dates" open={open} {...(fold === undefined ? {} : { fold })} testId="dates">
      <BoundRow view={view} node={key} label="Due" bound={dates.due} />
      <Actuals view={view} detail={detail} />
      <details className="date-more" data-testid="more-dates">
        <summary className="muted small">Earliest start, latest start and slack</summary>
        <div className="stack">
          <BoundRow view={view} node={key} label="Earliest start" bound={dates.earliest_start} />
          <BoundRow view={view} node={key} label="Latest start" bound={dates.latest_start} />
          {dates.effective_date == null ? null : (
            <span data-testid="effective-date">
              <span className="date-label">Effective date</span> <strong>{dateWords(dates.effective_date.date, view.derived.today)}</strong>{" "}
              <span className="muted small">{originWord(dates.effective_date.origin)}</span>
            </span>
          )}
          <span data-testid="slack">
            <span className="date-label">Slack</span> {dates.slack_days == null ? "no deadline" : `${String(dates.slack_days)} days`}
          </span>
        </div>
      </details>
      {dates.shortfall == null ? null : <ShortfallView view={view} short={dates.shortfall} onMove={onMove} />}
      {pinEditor}
    </Section>
  );
}
