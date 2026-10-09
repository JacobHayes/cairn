// What a save leaves under the control that made it (design 9.4): "Saved", and one sentence
// only when the write has warning consequences ("Kickoff is now overdue."). Everything else
// the save caused shows on the nodes themselves and in the sync chip's Recent. Surfaces that
// commit an edit (the inspector, the Next page) hold a `ReceiptState` and render this.
import { useEffect, useState } from "react";

export interface ReceiptState {
  warning: string | undefined;
}

/** How long a receipt stays under its control; nothing on it needs closing. */
export const RECEIPT_MS = 8000;

/** A receipt that clears itself: `show` it when a write lands, `clear` it when a new one starts. */
export function useReceipt(): { receipt: ReceiptState | undefined; show: (warning: string | undefined) => void; clear: () => void } {
  const [receipt, setReceipt] = useState<ReceiptState | undefined>();
  useEffect(() => {
    if (receipt === undefined) {
      return undefined;
    }
    const timer = setTimeout(() => {
      setReceipt(undefined);
    }, RECEIPT_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [receipt]);
  return {
    receipt,
    show: (warning) => {
      setReceipt({ warning });
    },
    clear: () => {
      setReceipt(undefined);
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
