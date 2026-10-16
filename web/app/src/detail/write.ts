// Node detail's writes: each action one patch through the edit manager (4.6: the safe retry,
// D7's consequences notice, nothing sent under version skew), drafted against the revision its
// author saw. The write is the manager's, found again by the section's edit key: a rejection
// waits there across a screen left and a reload, and the section shows it inline when it is on
// screen (A15) and the sync chip counts it wherever the person is. Each section holds its own.
import { useDraft } from "../data/drafts.ts";
import type { Command, OperationId } from "../data/edit-record.ts";
import { held } from "../data/edit-manager.ts";
import { useEdit, useSession, useSkew } from "../data/react.ts";
import type { WriteView } from "../data/write-machine.ts";
import { landedWarning, newPatchId, patchOf, type Rejection } from "../data/writes.ts";
import { useReceipt } from "../ui/Receipt.tsx";
import type { ReceiptState } from "../ui/receipt.ts";
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

/** The edit a section's writes are, found again by it wherever the section is shown: one write of it is out at a time. */
export function nodeEdit(journey: string, scope: string): string {
  return `node:${journey}:${scope}`;
}

/** The patch a section sends: `mutations` of `journey` drafted against `seen`, under the patch id `id`. */
export function nodeCommand(journey: string, mutations: Mutation[], seen: Seen, id: OperationId = newPatchId()): Command {
  const patch = patchOf({ target: { journey }, baseRevision: seen.base, mutations, ...(writesEntities(mutations) ? { deploymentRevision: seen.deployment } : {}) }, id);
  return { kind: "patch", patch };
}

/** What a held write drafted: its mutations and the revisions they were drafted against (the view's deployment when it named none). */
export function attemptOf(command: Command, deployment: number): Attempt | undefined {
  if (command.kind !== "patch") {
    return undefined;
  }
  const { patch } = command;
  return { mutations: patch.mutations, base: patch.base_revision, deployment: patch.deployment_revision ?? deployment };
}

/**
 * A section's write path for the journey `view` shows, as the edit `scope` (the section and
 * its node) names. Its writes are the edit manager's, so a rejection and the resolution being
 * chosen survive leaving the screen and a reload, and the sync chip counts it, named by
 * `node`'s title (the one the section is about), wherever the person is.
 */
export function useNodeWrite(view: Ready, scope: string, node?: string): NodeWrite {
  const session = useSession();
  const { edits } = session;
  const skew = useSkew();
  const journey = view.journey.header.id;
  const edit = useEdit(nodeEdit(journey, scope));
  const revision = view.journey.revision;
  const deployment = view.key.deployment_revision;
  const current: Seen = { base: revision, deployment };
  const label = node === undefined ? view.journey.header.name : titleOf(view, node);
  const receipt = useReceipt();
  const run = async (mutations: Mutation[], seen: Seen = current, id: OperationId = newPatchId()): Promise<boolean> => {
    if (skew) {
      return false;
    }
    receipt.clear();
    const answer = await edit.submit(nodeCommand(journey, mutations, seen, id), { label });
    if (answer !== undefined) {
      receipt.show(landedWarning(answer, session.titleOf));
    }
    return answer !== undefined;
  };
  const attempt = edit.write === undefined ? undefined : attemptOf(edit.write.command, deployment);
  return {
    journey,
    revision,
    seen: current,
    run,
    holds: (id) => held(edits, id),
    status: edit.status,
    pending: edit.busy,
    disabled: edit.busy || skew,
    failed: edit.write?.state === "rejected" && attempt !== undefined ? { id: edit.write.id, attempt, rejection: edit.write.rejection } : undefined,
    dismiss: edit.dismiss,
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
