// C11: the decision walkthrough with no decision to act on says so, and shows the milestones and
// dependencies that would unblock the earliest ones (waiting.ts), each with its own action when
// it is on the acting frontier. Continue with everything turns the walkthrough off and keeps
// the pass.
import { useLocation } from "react-router";

import { useProjected } from "../canvas/hooks.ts";
import { namer, viaText } from "../detail/explain.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { nodePath, screenPath } from "../detail/parts.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { DetailLink } from "./Parts.tsx";
import { RowActions } from "./RowActions.tsx";
import { WAITING_SHOWN_MAX, waitingDecisions, type Unblocker } from "./waiting.ts";
import type { NodeRow } from "./why.ts";

function UnblockerLine({ view, unblocker, row }: { view: Ready; unblocker: Unblocker; row: NodeRow | undefined }) {
  const { pathname, search } = useLocation();
  const name = namer(view);
  return (
    <li className="stack unblocker" data-testid="unblocker" data-node={unblocker.node} data-kind={unblocker.kind}>
      <span>
        <Badge tone={unblocker.kind === "milestone" ? "warn" : "plain"}>{unblocker.kind}</Badge> <DetailLink view={view} node={unblocker.node} />{" "}
        <span className="muted small">
          ({unblocker.via === "snooze" ? "it is snoozed until this is finished" : viaText(unblocker.via, name)}
          {unblocker.through === undefined ? "" : ` of ${titleOf(view, unblocker.through)}, which holds it`})
        </span>
      </span>
      {row === undefined ? null : <RowActions view={view} row={row} to={`${nodePath(screenPath(pathname), unblocker.node)}${search}`} />}
    </li>
  );
}

export function WaitingDecisions({ view, onContinue }: { view: Ready; onContinue: () => void }) {
  const waiting = waitingDecisions(view);
  const { value: next } = useProjected(view, { projection: "next" });
  const rows = new Map((next?.items ?? []).map((row) => [row.key, row]));
  const proceed = <Button onClick={onContinue}>Continue with everything →</Button>;
  if (waiting.length === 0) {
    return (
      <div className="callout stack" data-testid="no-decisions">
        <strong>No more decisions can be made now.</strong>
        <span>Every decision in scope is made or skipped.</span>
        <span>{proceed}</span>
      </div>
    );
  }
  return (
    <section className="callout stack" data-testid="waiting-decisions">
      <strong>No more decisions can be made now.</strong>
      <span>These would unblock the earliest ones:</span>
      <ul className="detail-list stack">
        {waiting.slice(0, WAITING_SHOWN_MAX).map((each) => (
          <li key={each.decision} className="stack" data-testid="waiting-decision" data-node={each.decision}>
            <span>
              <DetailLink view={view} node={each.decision} />
              <span className="muted small">
                {each.earliest === undefined ? "" : `, can start ${each.earliest}`}
                {each.snoozedUntil === undefined ? "" : `, snoozed until ${each.snoozedUntil}`}
              </span>
            </span>
            <ul className="detail-list stack">
              {each.unblockers.map((unblocker) => (
                <UnblockerLine key={unblocker.node} view={view} unblocker={unblocker} row={rows.get(unblocker.node)} />
              ))}
            </ul>
          </li>
        ))}
      </ul>
      {waiting.length > WAITING_SHOWN_MAX ? (
        <span className="muted small">
          And {waiting.length - WAITING_SHOWN_MAX} more open decisions, after {titleOf(view, waiting[WAITING_SHOWN_MAX - 1]?.decision ?? "")}.
        </span>
      ) : null}
      <span>{proceed}</span>
    </section>
  );
}
