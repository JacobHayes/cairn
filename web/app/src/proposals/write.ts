// Proposal writes from a screen (I6): each is a serializable command the edit manager owns
// (a write to a proposal, or one that drafts it), named by a fresh patch id (H5) and refused
// before anything is sent under version skew (ARCHITECTURE, Web UI). An apply records a save
// with what it caused (D7). The write is the manager's, found again by the control's edit key,
// so its rejection survives leaving the screen and a reload. A stale proposal is never rebased
// behind a person's back: it is rejected, and refreshed and reviewed again by them.
import type { Answer, ProposalCall, ProposalCommand } from "../data/edit-record.ts";
import { useEdit, useSession, useSkew } from "../data/react.ts";
import type { WriteView } from "../data/write-machine.ts";
import { isPatchAnswer, landedWarning, newPatchId, type Rejection } from "../data/writes.ts";
import { useReceipt } from "../ui/Receipt.tsx";
import type { ReceiptState } from "../ui/receipt.ts";

/** A proposal call before it has a patch id: what a control asks for. */
export type ProposalAsk = ProposalCall extends infer C ? (C extends { request: infer R } ? Omit<C, "request"> & { request: Omit<R, "patch_id"> } : never) : never;

/** The proposal command for `ask`, named by the patch id `id`. */
export function proposalCommand(ask: ProposalAsk, id: string = newPatchId()): ProposalCommand {
  // The ask is the call less the patch id its request is named by; the spread puts it back.
  return { kind: "proposal", call: { ...ask, request: { ...ask.request, patch_id: id } } as ProposalCall };
}

export interface ProposalWrite {
  /** Runs `ask` under a fresh patch id; its answer when it landed. */
  run: (ask: ProposalAsk) => Promise<Answer | undefined>;
  pending: boolean;
  disabled: boolean;
  /** The last write's rejection, until dismissed or a later write. */
  rejection: Rejection | undefined;
  /** What the write is doing when it is not rejected: still resending, not sent, or its subject gone. */
  status: WriteView | undefined;
  dismiss: () => void;
  /** Whether the control that sent the write is still mounted, for follow-ups that move the person. */
  mounted: () => boolean;
  /** What an apply leaves under its control, for a few seconds. */
  receipt: ReceiptState | undefined;
}

/** The proposal writes of the control `edit` names (a key stable for it), under the chip label `label`. */
export function useProposalWrite(edit: string, label?: string): ProposalWrite {
  const session = useSession();
  const skew = useSkew();
  const held = useEdit(edit);
  const receipt = useReceipt();
  const run = async (ask: ProposalAsk): Promise<Answer | undefined> => {
    receipt.clear();
    const answer = await held.submit(proposalCommand(ask), { label });
    if (answer !== undefined && isPatchAnswer(answer)) {
      receipt.show(landedWarning(answer, session.titleOf));
    }
    return answer;
  };
  return {
    run,
    pending: held.busy,
    disabled: held.busy || skew,
    rejection: held.rejection,
    status: held.status,
    dismiss: held.dismiss,
    mounted: held.mounted,
    receipt: receipt.receipt,
  };
}
