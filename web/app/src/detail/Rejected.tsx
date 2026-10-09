// A section's rejected write, shown inline against the patch it rejected (A15: every
// violation): a stale one with what intervened and a retry on the current version (H5); a
// contradictory chain with its resolution moves (F5, the DateConflictResolver); a failed
// guard that can be bypassed with a reason (D4); anything else as its violations.
import type { Schema } from "@cairn/client";

import { useDraft } from "../data/drafts.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { rebasedOnto } from "../screens/TitleEditor.tsx";
import { Button, Field } from "../ui/kit.tsx";
import { titleOf, type Mutation, type Ready } from "./model.ts";
import { ShortfallView } from "./ShortfallView.tsx";
import type { Attempt, NodeWrite, Seen } from "./write.ts";

type Violation = Schema<"Violation">;
type Guard = Schema<"Guard">;

interface Resolving {
  view: Ready;
  write: NodeWrite;
  /** Called once a retry of the rejected edit lands: the form that drafted it closes. */
  onResolved?: (() => void) | undefined;
}

/**
 * Sends the rejected edit again, changed, and reports when it lands. It keeps the revision
 * the author drafted against, so a change someone made meanwhile to what it touches is shown
 * as a conflict (H5), never overwritten.
 */
function retry(write: NodeWrite, mutations: Mutation[], seen: Seen, onResolved: (() => void) | undefined): void {
  void write.run(mutations, seen).then((landed) => {
    if (landed) {
      onResolved?.();
    }
  });
}

/** F5: the rejected patch with one resolution move after it: the edit kept, the chain given its days. */
export function withMove(attempt: Attempt, move: Mutation): Mutation[] {
  return [...attempt.mutations, move];
}

/** The node a violation is on, when it is on one: a patch over several nodes names which (C9). */
export function violatingNode(violation: Violation): string | undefined {
  const subject = violation.at.subject;
  return subject != null && typeof subject === "object" && "node" in subject ? subject.node : undefined;
}

/** D4: the guards a bypass would accept the rejected patch past, each once, on the node it is for. */
export function bypassable(violations: Violation[]): { node: string; guards: Guard[] }[] {
  const byNode = new Map<string, Guard[]>();
  for (const violation of violations) {
    const node = violatingNode(violation);
    if (violation.bypassable == null || node === undefined) {
      continue;
    }
    const guards = byNode.get(node) ?? [];
    if (!guards.includes(violation.bypassable)) {
      guards.push(violation.bypassable);
    }
    byNode.set(node, guards);
  }
  return [...byNode].map(([node, guards]) => ({ node, guards }));
}

/** F5: the contradictory chains a rejection lists, each with its moves, any one sent with the edit. */
export function DateConflictResolver({ view, write, violations, onResolved }: Resolving & { violations: Violation[] }) {
  const { failed } = write;
  const chains = violations.flatMap((violation) => violation.chains?.chains ?? []);
  if (failed === undefined || chains.length === 0) {
    return null;
  }
  const more = violations.some((violation) => violation.chains?.more === true);
  return (
    <div className="stack" data-testid="date-conflict">
      <strong>That date contradicts the plan: a chain needs more days than it allows.</strong>
      {chains.map((short, at) => (
        <ShortfallView
          key={at}
          view={view}
          short={short}
          disabled={write.disabled}
          onMove={(move) => { retry(write, withMove(failed.attempt, move), failed.attempt, onResolved); }}
        />
      ))}
      {more ? <span className="muted small">There are more chains than shown; resolve these first.</span> : null}
    </div>
  );
}

function Bypass({ write, node, guards, onResolved }: Omit<Resolving, "view"> & { node: string; guards: Guard[] }) {
  const [draft, setReason] = useDraft<string>(`bypass:${write.journey}:${node}`);
  const reason = draft ?? "";
  const { failed } = write;
  if (failed === undefined) {
    return null;
  }
  const override: Mutation = { op: "apply_override", node, override: { guard_bypass: { guards, reason } } };
  return (
    <div className="row" data-testid="bypass">
      <span>Do it anyway, bypassing {guards.join(", ")}:</span>
      <Field aria-label="Why bypass the guard" placeholder="Why" value={reason} onChange={(event) => { setReason(event.target.value); }} />
      <Button
        disabled={write.disabled || reason.trim() === ""}
        onClick={() => {
          retry(write, [override, ...failed.attempt.mutations], failed.attempt, () => {
            setReason(undefined);
            onResolved?.();
          });
        }}
      >
        Bypass
      </Button>
    </div>
  );
}

function Invalid({ view, write, violations, onResolved }: Resolving & { violations: Violation[] }) {
  const chains = violations.filter((violation) => violation.code === "contradictory_chain");
  const others = violations.filter((violation) => violation.code !== "contradictory_chain");
  return (
    <div className="callout callout-bad stack" role="alert" data-testid="invalid">
      {others.length === 0 ? null : (
        <ul className="detail-list">
          {others.map((violation, at) => {
            const node = violatingNode(violation);
            return (
              <li key={at} data-testid="violation" data-code={violation.code} data-node={node}>
                {node === undefined ? null : <strong>{titleOf(view, node)}: </strong>}
                {violation.message}
              </li>
            );
          })}
        </ul>
      )}
      <DateConflictResolver view={view} write={write} violations={chains} onResolved={onResolved} />
      {bypassable(violations).map(({ node, guards }) => (
        <Bypass key={node} write={write} node={node} guards={guards} onResolved={onResolved} />
      ))}
      <span className="row">
        <Button onClick={write.dismiss}>Drop this change</Button>
      </span>
    </div>
  );
}

/** The section's rejected write, if any, with what can be done about it. */
export function Rejected({ view, write, onResolved }: Resolving) {
  const { failed } = write;
  if (failed === undefined) {
    return null;
  }
  const { rejection, attempt } = failed;
  if (rejection.rejection === "invalid") {
    return <Invalid view={view} write={write} violations={rejection.violations} onResolved={onResolved} />;
  }
  return (
    <RejectionView
      rejection={rejection}
      onRebase={() => {
        // Kept on the current version: the journey revision the rejection reports, and the
        // deployment the view holds now, which the author sees with the conflict.
        const seen = { base: rebasedOnto(rejection, write.journey, write.revision), deployment: write.seen.deployment };
        retry(write, attempt.mutations, seen, onResolved);
      }}
    />
  );
}
