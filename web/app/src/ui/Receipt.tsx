// What a save leaves under the control that made it (design 9.4): "Saved", and one sentence
// only when the write has warning consequences ("Kickoff is now overdue."). Everything else
// the save caused shows on the nodes themselves and in the sync chip's Recent. Surfaces that
// commit an edit (the inspector, the Next page) hold a `useReceipt` and render this; its
// timing is the receipt machine's (receipt.ts).
import { useActorRef, useSelector } from "@xstate/react";

import { receiptMachine, receiptOf, type ReceiptState } from "./receipt.ts";

/** A receipt that clears itself: `show` it when a write lands, `clear` it when a new one starts. Its lifetime is the control's. */
export function useReceipt(): { receipt: ReceiptState | undefined; show: (warning: string | undefined) => void; clear: () => void } {
  const ref = useActorRef(receiptMachine);
  const receipt = useSelector(ref, receiptOf);
  return {
    receipt,
    show: (warning) => {
      ref.send({ type: "show", warning });
    },
    clear: () => {
      ref.send({ type: "clear" });
    },
  };
}

export function Receipt({ receipt }: { receipt: ReceiptState | undefined }) {
  if (receipt === undefined) {
    return null;
  }
  return (
    <span className="receipt stack" data-testid="receipt">
      <span>{"✓"} Saved</span>
      {receipt.warning === undefined ? null : (
        <span className="small" data-testid="receipt-warning">
          {receipt.warning}
        </span>
      )}
    </span>
  );
}

/**
 * Whether a form's own write is out: `saving` while it is on its way, `waiting` once it needs more
 * than that (a retry or the person), which the write's own status then says. Closing the form
 * leaves either one going.
 */
export type Sending = false | "saving" | "waiting";

/** Under a form whose write is on its way. */
export function Saving({ sending }: { sending: Sending }) {
  return sending === "saving" ? (
    <span className="muted small" data-testid="saving">
      Saving...
    </span>
  ) : null;
}
