// C8, F7: a node's dates, each told apart as pin, actual, or derived, each with the chain that
// produced it (conditional links labeled) and the pins and rules to edit to change it; slack;
// a shortfall with its chain and the moves that resolve it (F5, F6). Everything comes from
// the local derive. What editing a pin looks like is the caller's (`pinEditor`), since it
// routes through a feeding decision when there is one (E3).
import type { ReactNode } from "react";

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

function BoundRow({ view, node, label, bound }: { view: Ready; node: string; label: string; bound: Bound | null | undefined }) {
  if (bound == null) {
    return (
      <div className="date-row" data-testid="date" data-label={label} data-origin="none">
        <span className="date-label">{label}</span> <span className="muted small">none: no date reaches it</span>
      </div>
    );
  }
  const origin = boundOrigin(bound, node);
  return (
    <div className="date-row" data-testid="date" data-label={label} data-origin={origin}>
      <span className="date-label">{label}</span> <strong>{bound.date}</strong> <span className="badge">{origin}</span>
      <details>
        <summary className="muted small">Why</summary>
        <ChainView view={view} chain={bound.chain} />
      </details>
    </div>
  );
}

function Actuals({ detail }: { detail: NodeDetail }) {
  const { started_on: started, finished_on: finished } = detail.record;
  if (started == null && finished == null) {
    return null;
  }
  return (
    <div className="row" data-testid="actuals">
      {started == null ? null : <span>Started <strong>{started}</strong> <span className="badge">actual</span></span>}
      {finished == null ? null : <span>Finished <strong>{finished}</strong> <span className="badge">actual</span></span>}
    </div>
  );
}

function summaryOf(detail: NodeDetail): string {
  const { dates } = detail.derived;
  const parts = [
    dates.due == null ? "no due date" : `due ${dates.due.date}`,
    dates.slack_days == null ? undefined : `slack ${String(dates.slack_days)} days`,
    dates.shortfall == null ? undefined : `${String(dates.shortfall.shortfall_days)} days short`,
  ];
  return parts.filter((part) => part !== undefined).join(", ");
}

export function DatesSection({
  view,
  detail,
  pinEditor,
  onMove,
}: {
  view: Ready;
  detail: NodeDetail;
  pinEditor?: ReactNode;
  onMove?: (move: Mutation) => void;
}) {
  const { dates } = detail.derived;
  const key = detail.node.key;
  return (
    <Section title="Dates" summary={summaryOf(detail)} open testId="dates">
      {dates.effective_date == null ? null : (
        <span data-testid="effective-date">
          Effective date <strong>{dates.effective_date.date}</strong> <span className="badge">{dates.effective_date.origin}</span>
        </span>
      )}
      <BoundRow view={view} node={key} label="Earliest start" bound={dates.earliest_start} />
      <BoundRow view={view} node={key} label="Latest start" bound={dates.latest_start} />
      <BoundRow view={view} node={key} label="Due" bound={dates.due} />
      <span data-testid="slack">
        Slack: {dates.slack_days == null ? "no deadline" : `${String(dates.slack_days)} days`}
      </span>
      <Actuals detail={detail} />
      {dates.shortfall == null ? null : <ShortfallView view={view} short={dates.shortfall} onMove={onMove} />}
      {pinEditor}
    </Section>
  );
}
