// Node detail's writes: each action one patch through the edit manager (4.6: the safe retry,
// D7's consequences notice, nothing sent under version skew), drafted against the revision its
// author saw. The write is the manager's, keyed by the section's edit: a rejection waits there
// across a screen left and a reload, and the section shows it inline when it is on screen (A15)
// and the sync chip counts it wherever the person is.
import { waitFor } from "xstate";

import { consequenceLines, warningOf } from "../data/activity.ts";
import { held, submitPatch } from "../data/edit-manager.ts";
import { useSession, useSkew, useWriteOf } from "../data/react.ts";
import { currentAddress } from "../data/session.ts";
import { useDraft } from "../data/drafts.ts";
import type { Patch } from "../data/host.ts";
import type { OperationId } from "../data/edit-record.ts";
import { writeView, type WriteView } from "../data/write-machine.ts";
import { newPatchId, patchOf, type Rejection } from "../data/writes.ts";
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

/** A rejected write, with the patch it refused. */
export interface Failed {
  /** The rejected write's operation id. */
  id: OperationId;
  attempt: Attempt;
  rejection: Rejection;
}

export interface NodeWrite {
  journey: string;
  /** The revision the view holds. */
  revision: number;
  /** What the view holds: what a fresh edit is drafted against. */
  seen: Seen;
  /**
   * Sends `mutations` drafted against `seen` (the view's by default) as the write `id` (a fresh
   * one by default; a caller that must find its write again mints it); true when it landed.
   * The write is submitted before the first await, so `holds(id)` answers at once.
   */
  run: (mutations: Mutation[], seen?: Seen, id?: OperationId) => Promise<boolean>;
  /** Whether the edit manager accepted the write `id` and still holds it. */
  holds: (id: OperationId) => boolean;
  /** The write the manager holds for this section, if it is not rejected: out, or in need of the person (no answer, not sent, gone). */
  status: WriteView | undefined;
  /** A write is out or may have committed. */
  pending: boolean;
  /** Version skew or a write in flight: no new write starts. */
  disabled: boolean;
  /** The last attempt's rejection, until dismissed or a later attempt. */
  failed: Failed | undefined;
  /** Drops the section's rejected (or stopped, or gone) write; one that is out or may have committed is never dropped. */
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

/** What `patch` was drafted against; a patch that names no deployment revision was drafted against `deployment`. */
export function attemptOf(patch: Patch, deployment: number): Attempt {
  return { mutations: patch.mutations, base: patch.base_revision, deployment: patch.deployment_revision ?? deployment };
}

/** The edit a section's writes share: one write of it is out at a time. */
export function nodeEdit(journey: string, scope: string): string {
  return `node:${journey}:${scope}`;
}

/**
 * A section's write path for the journey `view` shows. Its writes are the edit manager's, under
 * the edit `scope` names (the section and its node), so a rejection and the resolution being
 * chosen survive a reload, and the sync chip counts it, named by `node`'s title (the one the
 * section is about), wherever the person is.
 */
export function useNodeWrite(view: Ready, scope: string, node?: string): NodeWrite {
  const session = useSession();
  const { edits } = session;
  const skew = useSkew();
  const journey = view.journey.header.id;
  const edit = nodeEdit(journey, scope);
  const written = useWriteOf(edit);
  const revision = view.journey.revision;
  const deployment = view.key.deployment_revision;
  const current: Seen = { base: revision, deployment };
  const label = node === undefined ? view.journey.header.name : titleOf(view, node);
  const receipt = useReceipt();
  const pending = written?.state === "sending" || written?.state === "unconfirmed";
  const run = async (mutations: Mutation[], seen: Seen = current, id: OperationId = newPatchId()): Promise<boolean> => {
    if (skew) {
      return false;
    }
    const patch = patchOf(
      { target: { journey }, baseRevision: seen.base, mutations, ...(writesEntities(mutations) ? { deploymentRevision: seen.deployment } : {}) },
      id,
    );
    // Where the edit was made, taken before the wait: the user may be elsewhere when the answer comes.
    const address = currentAddress();
    const ref = submitPatch(edits, patch, { edit, label, ...(address === undefined ? {} : { address }) });
    if (ref === undefined) {
      return false;
    }
    receipt.clear();
    let snapshot;
    try {
      snapshot = await waitFor(ref, (each) => each.status === "done" || each.hasTag("needsYou"), { timeout: Infinity });
    } catch {
      // The session closed.
      return false;
    }
    const result = writeView(snapshot);
    if (result.state !== "landed") {
      return false;
    }
    receipt.show(warningOf(consequenceLines(result.answer), session.titleOf));
    return true;
  };
  return {
    journey,
    revision,
    seen: current,
    run,
    holds: (id) => held(edits, id),
    status: written?.state === "rejected" ? undefined : written,
    pending,
    disabled: pending || skew,
    failed: written?.state === "rejected" ? { id: written.id, attempt: attemptOf(written.command.patch, deployment), rejection: written.rejection } : undefined,
    dismiss: () => {
      if (written?.state === "rejected" || written?.state === "stopped" || written?.state === "gone") {
        edits.send({ type: "discard", id: written.id });
      }
    },
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

/**
 * The revision an author who keeps their edit after a conflict drafts it against: the
 * journey's revision the rejection reports (only what it shows was compared, H5), or the one
 * the view holds if it has moved further.
 */
export function rebasedOnto(rejection: Rejection, journey: string, revision: number): number {
  if (rejection.rejection !== "stale") {
    return revision;
  }
  const reported = rejection.conflicts.map((conflict) =>
    "domain" in conflict.of && typeof conflict.of.domain === "object" && "journey" in conflict.of.domain && conflict.of.domain.journey === journey
      ? conflict.current
      : 0,
  );
  return Math.max(revision, ...reported);
}
