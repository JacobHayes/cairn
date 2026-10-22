// A section's rejected write, shown inline against the patch it rejected (A15: every
// violation): a stale one with what intervened and a retry on the current version (H5); a
// contradictory chain with its resolution moves (F5, the DateConflictResolver); a failed
// guard that can be bypassed with a reason (D4); anything else as its violations.
import type { Schema } from "@cairn/client";
import { useState } from "react";

import { useDraft } from "../data/drafts.ts";
import type { OperationId } from "../data/edit-record.ts";
import { newPatchId, violationNode, type Rejection, type Violation } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Button, Field } from "../ui/kit.tsx";
import { plural } from "../ui/words.ts";
import { Receipt } from "../ui/Receipt.tsx";
import { checklistOf, guardBypass, itemTitle, titleOf, type Mutation, type Ready } from "./model.ts";
import { ShortfallView } from "./ShortfallView.tsx";
import { guardWords } from "./words.ts";
import { rebasedOnto, type Attempt, type Failed, type NodeWrite, type Seen } from "./write.ts";

type Guard = Schema<"Guard">;

interface Resolving {
  view: Ready;
  write: NodeWrite;
  /** Called once a retry of the rejected edit lands: the form that drafted it closes. */
  onResolved?: (() => void) | undefined;
  /** Called when a retry is sent in the place of the rejected write `from`: the form that drafted it follows the new write `id`. */
  onResent?: ((from: OperationId, id: OperationId) => void) | undefined;
}

/**
 * Sends the rejected edit again, changed, and reports when it lands. It keeps the revision
 * the author drafted against, so a change someone made meanwhile to what it touches is shown
 * as a conflict (H5), never overwritten. Only a landing resolves it: another rejection leaves
 * the form that drafted it open.
 */
function retry(write: NodeWrite, failed: Failed, mutations: Mutation[], seen: Seen, { onResolved, onResent }: Pick<Resolving, "onResolved" | "onResent">): void {
  const id = newPatchId();
  void write.run(mutations, seen, id).then((landed) => {
    if (landed) {
      onResolved?.();
    }
  });
  if (write.holds(id)) {
    onResent?.(failed.id, id);
  }
}

/** F5: the rejected patch with one resolution move after it: the edit kept, the chain given its days. */
export function withMove(attempt: Attempt, move: Mutation): Mutation[] {
  return [...attempt.mutations, move];
}

/** D4: finishing past a guard takes a reason, so the bypass waits until one is typed. */
export function bypassReady(reason: string): boolean {
  return reason.trim() !== "";
}

/** D4: the guards a bypass would accept the rejected patch past, each once, on the node it is for. */
export function bypassable(violations: Violation[]): { node: string; guards: Guard[] }[] {
  const byNode = new Map<string, Guard[]>();
  for (const violation of violations) {
    const node = violationNode(violation);
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

/** G5, D4: the unticked items a refusal names, per node, and whether ticking them is all it needs. */
export function untickedIn(violations: Violation[]): { byNode: { node: string; items: string[] }[]; only: boolean } {
  const byNode = new Map<string, string[]>();
  for (const violation of violations) {
    const node = violationNode(violation);
    if (violation.bypassable !== "all_ticked" || node === undefined) {
      continue;
    }
    const items = byNode.get(node) ?? [];
    for (const failure of violation.failures ?? []) {
      if (typeof failure === "object" && "unticked" in failure && !items.includes(failure.unticked)) {
        items.push(failure.unticked);
      }
    }
    byNode.set(node, items);
  }
  const only = violations.length > 0 && violations.every((violation) => violation.bypassable === "all_ticked");
  return { byNode: [...byNode].map(([node, items]) => ({ node, items })), only };
}

/** The rejected patch with each named item ticked first, so the guard reads them ticked. */
export function withTicks(attempt: Attempt, byNode: { node: string; items: string[] }[]): Mutation[] {
  const ticks = byNode.flatMap(({ node, items }) => items.map((item) => ({ op: "set_ticked" as const, node, item, ticked: true })));
  return [...ticks, ...attempt.mutations];
}

/** F5: the contradictory chains a rejection lists, each with its moves, any one sent with the edit. */
export function DateConflictResolver({ view, write, violations, onResolved, onResent }: Resolving & { violations: Violation[] }) {
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
          onMove={(move) => { retry(write, failed, withMove(failed.attempt, move), failed.attempt, { onResolved, onResent }); }}
        />
      ))}
      {more ? <span className="muted small">There are more chains than shown; resolve these first.</span> : null}
    </div>
  );
}

type Bypassed = { node: string; guards: Guard[] }[];

/** The completions a retried patch carries, in words: "mark done", "mark both done", "mark all 3 done". */
function completionWords(attempt: Attempt): string {
  const completing = attempt.mutations.filter((mutation) => mutation.op === "transition" && mutation.transition === "complete").length;
  return completing === 0 ? "go ahead" : completing === 1 ? "mark done" : completing === 2 ? "mark both done" : `mark all ${String(completing)} done`;
}

/** D4: the bypass of every node's failed guards at once, one reason for the lot, named by what it does; its reason field opens only when the person chooses it, after the remedy above has been offered. */
function Bypass({ write, entries, onResolved, onResent }: Resolving & { entries: Bypassed }) {
  const nodes = entries.map(({ node }) => node);
  const [draft, setReason] = useDraft<string>(`rejected-bypass:${write.journey}:${nodes.join(",")}`);
  const [chosen, choose] = useState(false);
  const reason = draft ?? "";
  const { failed } = write;
  if (failed === undefined) {
    return null;
  }
  const overrides = entries.map(({ node, guards }) => guardBypass(node, guards, reason));
  const verb = completionWords(failed.attempt).replace(/^./, (first) => first.toUpperCase());
  const without = [...new Set(entries.flatMap(({ guards }) => guards))].map(guardWords).join(" and ");
  if (!chosen && reason === "") {
    return (
      <span data-testid="bypass-offer">
        <Button onClick={() => { choose(true); }}>
          {`${verb} without ${without}\u2026`}
        </Button>
      </span>
    );
  }
  return (
    <div className="stack" data-testid="bypass">
      <label className="stack">
        <span className="label">{`Why ${verb.toLowerCase()} without ${without}`}</span>
        <Field value={reason} onChange={(event) => { setReason(event.target.value); }} />
      </label>
      <span className="row">
        <Button
          disabled={write.disabled || !bypassReady(reason)}
          onClick={() => {
            retry(write, failed, [...overrides, ...failed.attempt.mutations], failed.attempt, {
              onResolved: () => {
                setReason(undefined);
                onResolved?.();
              },
              onResent,
            });
          }}
        >
          {`${verb} with this reason`}
        </Button>
        <Button onClick={() => { setReason(undefined); choose(false); }}>Cancel</Button>
      </span>
    </div>
  );
}

/** G5: a node's unticked items by title, in the order of its checklist, as the done form lists them. */
function TickList({ view, node, items }: { view: Ready; node: string; items: readonly string[] }) {
  const order = checklistOf(view, node).map((item) => item.id);
  const inOrder = [...items].sort((left, right) => (order.indexOf(left) + 1 || order.length + 1) - (order.indexOf(right) + 1 || order.length + 1));
  return (
    <ul className="done-ticks-list">
      {inOrder.map((item) => (
        <li key={item}>{itemTitle(view, node, item)}</li>
      ))}
    </ul>
  );
}

/** G5: the checklist items a refusal names, and the one button that ticks them and sends the refused patch again. */
function TickTheRest({ view, write, failed, ticks, only, onResolved, onResent }: Resolving & { failed: Failed; ticks: { node: string; items: string[] }[]; only: boolean }) {
  const total = ticks.reduce((sum, { items }) => sum + items.length, 0);
  return (
    <>
      {only ? (
        <span>
          <Button
            primary
            disabled={write.disabled}
            onClick={() => { retry(write, failed, withTicks(failed.attempt, ticks), failed.attempt, { onResolved, onResent }); }}
          >
            {`Tick ${plural(total, "item")} and ${completionWords(failed.attempt)}`}
          </Button>
        </span>
      ) : null}
      {ticks.map(({ node, items }) => (
        <div className="stack" key={node} data-testid="tick-the-rest" data-node={node}>
          <span>
            <strong>{titleOf(view, node)}</strong>:
          </span>
          <TickList view={view} node={node} items={items} />
        </div>
      ))}
    </>
  );
}

function Invalid({ view, write, failed, rejection, violations, onResolved, onResent }: Resolving & { failed: Failed; rejection: Rejection; violations: Violation[] }) {
  const chains = violations.filter((violation) => violation.code === "contradictory_chain");
  const ticks = untickedIn(violations);
  const bypasses = bypassable(violations);
  return (
    <RejectionView
      rejection={rejection}
      titleOf={(node) => titleOf(view, node)}
      omit={(violation) => violation.code === "contradictory_chain" || violation.bypassable === "all_ticked"}
      {...(ticks.only ? { invalidHeading: "checklist items are not ticked." } : {})}
      onDismiss={write.dismiss}
      extras={
        <>
          <DateConflictResolver view={view} write={write} violations={chains} onResolved={onResolved} onResent={onResent} />
          <TickTheRest view={view} write={write} failed={failed} ticks={ticks.byNode} only={ticks.only} onResolved={onResolved} onResent={onResent} />
          {bypasses.length === 0 ? null : <Bypass write={write} view={view} entries={bypasses} onResolved={onResolved} onResent={onResent} />}
        </>
      }
    />
  );
}

/**
 * What the section's last write left under it: its rejection, with what can be done about it;
 * else what a write that did not land needs of the person (no answer, not sent, gone), and the
 * receipt of one that did.
 */
export function Rejected({ view, write, onResolved, onResent }: Resolving) {
  const { failed } = write;
  if (failed === undefined) {
    return (
      <>
        <WriteStatus view={write.status} />
        <Receipt receipt={write.receipt} />
      </>
    );
  }
  const { rejection, attempt } = failed;
  if (rejection.rejection === "invalid") {
    return <Invalid view={view} write={write} failed={failed} rejection={rejection} violations={rejection.violations} onResolved={onResolved} onResent={onResent} />;
  }
  return (
    <RejectionView
      rejection={rejection}
      onRebase={() => {
        // Kept on the current version: the journey revision the rejection reports, and the
        // deployment the view holds now, which the author sees with the conflict.
        const seen = { base: rebasedOnto(rejection, write.journey, write.revision), deployment: write.seen.deployment };
        retry(write, failed, attempt.mutations, seen, { onResolved, onResent });
      }}
    />
  );
}
