// The write path for the screens around a journey (the index, the overview, the route
// screens, entities): one patch through the shell's write path (H5's safe retry, D7's
// save, nothing sent under version skew), with the rejection kept to show inline (A15) and
// counted on the sync chip until it is dismissed or the screen goes.
import type { Schema } from "@cairn/client";
import { useEffect, useId, useState } from "react";

import { noticesOf } from "../data/activity.ts";
import { unlandedOf, useProblem, useSession, useSkew } from "../data/react.ts";
import type { Rejection, WriteIntent } from "../data/writes.ts";
import { useReceipt, type ReceiptState } from "../ui/Receipt.tsx";

export interface ScreenWrite {
  /** Sends `intent`; true when it landed, in which case `onLanded` is told the advisory notices (A20) its answer carried. */
  run: (intent: WriteIntent, onLanded?: (notices: Schema<"Notice">[]) => void) => Promise<boolean>;
  pending: boolean;
  /** Version skew or a write in flight: no new write starts. */
  disabled: boolean;
  /** The last attempt's rejection, until dismissed or a later attempt. */
  rejected: Rejection | undefined;
  dismiss: () => void;
  /** What the last landed write leaves under the control, for a few seconds. */
  receipt: ReceiptState | undefined;
}

export function useScreenWrite(): ScreenWrite {
  const session = useSession();
  const skew = useSkew();
  const [pending, setPending] = useState(false);
  const [rejected, setRejected] = useState<Rejection | undefined>();
  const { sync } = session;
  const key = `screen:${useId()}`;
  const dismiss = () => {
    setRejected(undefined);
  };
  useProblem(key, rejected === undefined ? undefined : unlandedOf(rejected), { discard: dismiss });
  // What this screen's state held goes with it, so the chip does not keep a rejection nobody can see.
  useEffect(() => () => { sync.resolve(key); }, [sync, key]);
  const receipt = useReceipt();
  const run = async (intent: WriteIntent, onLanded?: (notices: Schema<"Notice">[]) => void): Promise<boolean> => {
    setPending(true);
    receipt.clear();
    const result = await session.write(intent);
    setPending(false);
    setRejected(result.outcome === "rejected" ? result.rejection : undefined);
    if (result.outcome === "landed") {
      receipt.show(result.warning);
      onLanded?.(noticesOf(result.answer));
    }
    return result.outcome === "landed";
  };
  return {
    run,
    pending,
    disabled: pending || skew !== undefined,
    rejected,
    dismiss,
    receipt: receipt.receipt,
  };
}
