// C8, F7: a node's dates, each told apart as pin, actual, or derived, each with the chain that
// produced it (conditional links labeled) and the pins and rules to edit to change it; slack;
// a shortfall with its chain and the moves that resolve it (F5, F6). Everything comes from
// the local derive. What editing a pin looks like is the caller's (`pinEditor`), since it
// routes through a feeding decision when there is one (E3).
import type { ReactNode } from "react";

import { dateWords } from "../timeline/model.ts";
import { ChainView } from "./ChainView.tsx";
import type { Bound } from "./explain.ts";
import type { Mutation, NodeDetail, Ready } from "./model.ts";
import { Section } from "./parts.tsx";
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

/** Where a date comes from, in quiet words: the origin attribute keeps the engine's term. */
const ORIGIN_WORDS: Record<BoundOrigin | "due", string> = { pin: "pinned", actual: "recorded", derived: "worked out", due: "worked out" };

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
      <span className="date-label">{label}</span> <strong>{dateWords(bound.date, view.derived.today)}</strong> <span className="muted small">{ORIGIN_WORDS[origin]}</span>
      <details>
        <summary className="muted small">Why</summary>
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
  onMove?: (move: Mutation) => void;
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
        <summary className="muted small">More dates</summary>
        <div className="stack">
          <BoundRow view={view} node={key} label="Earliest start" bound={dates.earliest_start} />
          <BoundRow view={view} node={key} label="Latest start" bound={dates.latest_start} />
          {dates.effective_date == null ? null : (
            <span data-testid="effective-date">
              <span className="date-label">Effective date</span> <strong>{dateWords(dates.effective_date.date, view.derived.today)}</strong>{" "}
              <span className="muted small">{ORIGIN_WORDS[dates.effective_date.origin]}</span>
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
