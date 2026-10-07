// C11: the decision walkthrough with no decision to act on shows the milestones and
// dependencies that would unblock the earliest ones (waiting.ts).
import { namer, viaText } from "../detail/explain.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { Badge } from "../ui/kit.tsx";
import { DetailLink } from "./Parts.tsx";
import { WAITING_SHOWN_MAX, waitingDecisions, type Unblocker } from "./waiting.ts";

function UnblockerLine({ view, unblocker }: { view: Ready; unblocker: Unblocker }) {
  const name = namer(view);
  return (
    <li data-testid="unblocker" data-node={unblocker.node} data-kind={unblocker.kind}>
      <Badge tone={unblocker.kind === "milestone" ? "warn" : "plain"}>{unblocker.kind}</Badge> <DetailLink view={view} node={unblocker.node} />{" "}
      <span className="muted">
        ({unblocker.via === "snooze" ? "it is snoozed until this is finished" : viaText(unblocker.via, name)}
        {unblocker.through === undefined ? "" : ` of ${titleOf(view, unblocker.through)}, which holds it`})
      </span>
    </li>
  );
}

export function WaitingDecisions({ view }: { view: Ready }) {
  const waiting = waitingDecisions(view);
  if (waiting.length === 0) {
    return (
      <p className="callout" data-testid="no-decisions">
        No decision is left open: every one in scope is made or skipped.
      </p>
    );
  }
  return (
    <section className="callout stack" data-testid="waiting-decisions">
      <strong>No decision can be made right now.</strong>
      <span>What would unblock the earliest ones:</span>
      <ul className="detail-list stack">
        {waiting.slice(0, WAITING_SHOWN_MAX).map((each) => (
          <li key={each.decision} className="stack" data-testid="waiting-decision" data-node={each.decision}>
            <span>
              <DetailLink view={view} node={each.decision} />
              <span className="muted">
                {each.earliest === undefined ? "" : `, can start ${each.earliest}`}
                {each.snoozedUntil === undefined ? "" : `, snoozed until ${each.snoozedUntil}`}
              </span>
            </span>
            <ul className="detail-list">
              {each.unblockers.map((unblocker) => (
                <UnblockerLine key={unblocker.node} view={view} unblocker={unblocker} />
              ))}
            </ul>
          </li>
        ))}
      </ul>
      {waiting.length > WAITING_SHOWN_MAX ? (
        <span className="muted">
          And {waiting.length - WAITING_SHOWN_MAX} more open decisions, after {titleOf(view, waiting[WAITING_SHOWN_MAX - 1]?.decision ?? "")}.
        </span>
      ) : null}
    </section>
  );
}
