// What a save leaves under the control that made it as a lifecycle: shown when a
// write lands, gone after `RECEIPT_MS` (a delayed transition), shown afresh by the next save
// and cleared when a new write starts. The control that shows it owns it (`useReceipt`).
import { assign, setup, type SnapshotFrom } from "xstate";

export interface ReceiptState {
  warning: string | undefined;
}

/** How long a receipt stays under its control; nothing on it needs closing. */
export const RECEIPT_MS = 8000;

export type ReceiptEvent =
  /** A write landed: the receipt shows, with the sentence its consequences call for. */
  | { type: "show"; warning: string | undefined }
  /** A new write started, or the control let go of it. */
  | { type: "clear" };

export const receiptMachine = setup({
  types: { context: {} as { receipt: ReceiptState | undefined }, events: {} as ReceiptEvent },
  actions: {
    put: assign({ receipt: ({ event }) => ({ warning: (event as Extract<ReceiptEvent, { type: "show" }>).warning }) }),
    clear: assign({ receipt: undefined }),
  },
}).createMachine({
  id: "receipt",
  context: { receipt: undefined },
  initial: "hidden",
  states: {
    hidden: { entry: "clear", on: { show: { target: "shown", actions: "put" } } },
    // The next save restarts the full time.
    shown: { after: { [RECEIPT_MS]: "hidden" }, on: { show: { target: "shown", reenter: true, actions: "put" }, clear: "hidden" } },
  },
});

/** The receipt on show, if any. */
export const receiptOf = (snapshot: SnapshotFrom<typeof receiptMachine>): ReceiptState | undefined => snapshot.context.receipt;
