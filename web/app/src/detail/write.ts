// Node detail's writes: each action one patch through the shell's write path (4.6: the safe
// retry, D7's consequences notice, nothing sent under version skew), drafted against the
// revision its author saw, with the rejection kept for the section to show inline (A15).
// Each section holds its own, so a rejection shows beside the action that caused it.
import { useState } from "react";

import { useDraft } from "../data/drafts.ts";
import { currentAddress, unlandedOf, useProblem, useSession, useSkew } from "../data/react.ts";
import type { Rejection } from "../data/writes.ts";
import { useReceipt, type ReceiptState } from "../ui/Receipt.tsx";
import { titleOf, type Mutation, type Ready } from "./model.ts";

/**
 * What an edit was drafted against: the journey revision its author saw (H5) and the
 * deployment revision (E6: an entity chosen under one deployment revision is stale once a
 * merge moves it, never silently redirected).
 */
export interface Seen {
  base: number;
  deployment: number;
}

/** A patch as it was sent: its mutations and what they were drafted against. */
export interface Attempt extends Seen {
  mutations: Mutation[];
}

export interface NodeWrite {
  journey: string;
  /** The revision the view holds. */
  revision: number;
  /** What the view holds: what a fresh edit is drafted against. */
  seen: Seen;
  /** Sends `mutations` drafted against `seen` (the view's by default); true when it landed. */
  run: (mutations: Mutation[], seen?: Seen) => Promise<boolean>;
  pending: boolean;
  /** Version skew or a write in flight: no new write starts. */
  disabled: boolean;
  /** The last attempt's rejection, until dismissed or a later attempt. */
  failed: { attempt: Attempt; rejection: Rejection; address?: string | undefined } | undefined;
  dismiss: () => void;
  /** What the last landed write leaves under the control, for a few seconds. */
  receipt: ReceiptState | undefined;
}

/** E6: whether a patch writes an entity reference, so it names the deployment revision it saw. */
export function writesEntities(mutations: Mutation[]): boolean {
  return mutations.some(
    (mutation) =>
      (mutation.op === "answer" && ("entity" in mutation.value || "entity_list" in mutation.value)) ||
      (mutation.op === "set_participation" && Array.isArray(mutation.source)) ||
      mutation.op === "fill_role",
  );
}

/**
 * A section's write path for the journey `view` shows. Its last rejection is kept as a draft
 * under `scope` (the section and its node), so it, and the resolution being chosen, survive
 * a reload as typed text does. While it stands the sync chip counts it, named by `node`'s
 * title (the one the section is about).
 */
export function useNodeWrite(view: Ready, scope: string, node?: string): NodeWrite {
  const session = useSession();
  const skew = useSkew();
  const [pending, setPending] = useState(false);
  const journey = view.journey.header.id;
  const [failed, setFailed] = useDraft<NonNullable<NodeWrite["failed"]>>(`rejected:${journey}:${scope}`);
  const revision = view.journey.revision;
  const deployment = view.key.deployment_revision;
  const current: Seen = { base: revision, deployment };
  const receipt = useReceipt();
  const dismiss = () => {
    setFailed(undefined);
  };
  const report = useProblem(`rejected:${journey}:${scope}`, failed === undefined ? undefined : unlandedOf(failed.rejection, failed.address), {
    label: node === undefined ? view.journey.header.name : titleOf(view, node),
    discard: dismiss,
  });
  const run = async (mutations: Mutation[], seen: Seen = current): Promise<boolean> => {
    setPending(true);
    receipt.clear();
    // Where the edit was made, taken before the wait: the user may be elsewhere when the answer comes.
    const address = currentAddress();
    const result = await session.write({
      target: { journey },
      baseRevision: seen.base,
      mutations,
      ...(writesEntities(mutations) ? { deploymentRevision: seen.deployment } : {}),
    });
    setPending(false);
    const attempt: Attempt = { mutations, base: seen.base, deployment: seen.deployment };
    setFailed(result.outcome === "rejected" ? { attempt, rejection: result.rejection, address } : undefined);
    // The control may be gone by now (its draft is kept): the chip hears of the result either way.
    report(result.outcome === "rejected" ? unlandedOf(result.rejection, address) : undefined);
    if (result.outcome === "landed") {
      receipt.show(result.warning);
    }
    return result.outcome === "landed";
  };
  return {
    journey,
    revision,
    seen: current,
    run,
    pending,
    disabled: pending || skew !== undefined,
    failed,
    dismiss,
    receipt: receipt.receipt,
  };
}

/** A form's draft: what is typed and what its author saw when they opened it. */
export interface FormDraft<T> extends Seen {
  value: T;
}

/**
 * A form on one node kept across reloads (ARCHITECTURE, Web UI: drafts survive a reload),
 * keyed by journey, node, and form (the host is the draft store's). Undefined while closed.
 */
export function useFormDraft<T>(journey: string, node: string, form: string) {
  const [draft, setDraft] = useDraft<FormDraft<T>>(`${form}:${journey}:${node}`);
  return {
    draft,
    open: (value: T, seen: Seen) => {
      setDraft({ value, base: seen.base, deployment: seen.deployment });
    },
    change: (value: T) => {
      if (draft !== undefined) {
        setDraft({ ...draft, value });
      }
    },
    close: () => {
      setDraft(undefined);
    },
  };
}
