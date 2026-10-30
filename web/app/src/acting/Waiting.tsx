// C11: the decision walkthrough with no decision to act on says so, and shows the milestones and
// dependencies that would unblock the earliest ones (waiting.ts), each with its own action when
// it is on the acting frontier. Continue with everything turns the walkthrough off and keeps
// the pass.

import { OWNER_KIND } from "@cairn/client";

import { useProjected } from "../canvas/hooks.ts";
import { valueOf } from "../data/keyed-read.ts";
import { namer, viaText } from "../detail/explain.ts";
import { kindWord, type Ready, titleOf } from "../detail/model.ts";
import { useNodeAddresses } from "../journeys/address.ts";
import { useMineOf } from "../journeys/mine.ts";
import { SNOOZE_NODE_TAIL, snoozeEndWords } from "../status/snooze.ts";
import { dateWords } from "../status/when.ts";
import { Badge, Button } from "../ui/kit.tsx";
import { RowActions } from "./RowActions.tsx";
import { WAITING_SHOWN_MAX, waitingDecisions, type Unblocker } from "./waiting.ts";
import type { NodeRow } from "./why.ts";
import { NodeLink } from "../detail/parts.tsx";
import { Words } from "../ui/Words.tsx";

function UnblockerLine({ view, unblocker, row, yours }: { view: Ready; unblocker: Unblocker; row: NodeRow | undefined; yours: boolean }) {
  const addresses = useNodeAddresses();
  const name = namer(view);
  return (
    <li className="stack unblocker" data-testid="unblocker" data-node={unblocker.node} data-kind={unblocker.kind}>
      <span>
        <Badge tone={unblocker.kind === "milestone" ? "warn" : "plain"}>{kindWord(unblocker.kind, unblocker.stage)}</Badge> <NodeLink view={view} node={unblocker.node} />{" "}
        <span className="muted small">
          ({unblocker.via === "snooze" ? `it is snoozed until this${SNOOZE_NODE_TAIL}` : viaText(unblocker.via, name)}
          {unblocker.through === undefined ? "" : ` of ${titleOf(view, unblocker.through)}, which holds it`})
        </span>
      </span>
      {row === undefined ? null : <RowActions view={view} row={row} to={addresses.open(unblocker.node)} yours={yours} />}
    </li>
  );
}

export function WaitingDecisions({ view, onContinue }: { view: Ready; onContinue: () => void }) {
  const summary = valueOf(useProjected(view, { projection: "status_summary" }));
  const mine = useMineOf(view.journey.header.id);
  const next = valueOf(useProjected(view, { projection: "next" }));
  const rows = new Map((next?.items ?? []).map((row) => [row.key, row]));
  const proceed = <Button onClick={onContinue}>Continue with everything →</Button>;
  if (summary === undefined || mine.status !== "ready") {
    return null;
  }
  const waiting = waitingDecisions(view, summary);
  const owned = new Set(mine.entries.filter((entry) => entry.kinds.includes(OWNER_KIND.key)).map((entry) => entry.node));
  if (waiting.length === 0) {
    return (
      <div className="callout stack" data-testid="no-decisions">
        <strong>No more decisions can be made now.</strong>
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
              <NodeLink view={view} node={each.decision} />
              <span className="muted small">
                {each.earliest === undefined ? null : <>, can start <Words text={dateWords(each.earliest, view.derived.today)} /></>}
                {each.snoozedUntil === undefined ? null : <>, snoozed until <Words text={snoozeEndWords(view, { date: each.snoozedUntil })} /></>}
              </span>
            </span>
            <ul className="detail-list stack">
              {each.unblockers.map((unblocker) => (
                <UnblockerLine key={unblocker.node} view={view} unblocker={unblocker} row={rows.get(unblocker.node)} yours={owned.has(unblocker.node)} />
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
