// The write path for the screens around a journey (the index, the overview, the route
// screens, identities): one patch through the edit manager (H5's safe retry, D7's save, nothing
// sent under version skew). The write is the manager's, found again by the screen's edit key:
// its rejection shows inline (A15) and is counted on the sync chip until it is discarded or
// the next write of the edit replaces it, wherever the person has gone since.
import type { Schema } from "@cairn/client";

import { noticesOf } from "../data/activity.ts";
import type { Command } from "../data/edit-command.ts";
import { useWrite } from "../data/react.ts";
import type { WriteView } from "../data/write-machine.ts";
import { isPatchAnswer, newPatchId, patchOf, type Rejection, type WriteIntent } from "../data/writes.ts";
import type { ReceiptState } from "../ui/receipt.ts";

export interface ScreenWrite {
  /** Sends `intent`; true when it landed, in which case `onLanded` is told the advisory notices (A20) its answer carried. */
  run: (intent: WriteIntent, onLanded?: (notices: Schema<"Notice">[]) => void) => Promise<boolean>;
  pending: boolean;
  /** Version skew or a write in flight: no new write starts. */
  disabled: boolean;
  /** The last attempt's rejection, until dismissed or a later attempt. */
  rejection: Rejection | undefined;
  /** What the write is doing when it is not rejected: still resending, not sent, or its subject gone. */
  status: WriteView | undefined;
  dismiss: () => void;
  /** What the last landed write leaves under the control, for a few seconds. */
  receipt: ReceiptState | undefined;
}

/** The patch a screen sends for `intent`, under the patch id `id`. */
export function screenCommand(intent: WriteIntent, id: string = newPatchId()): Command {
  const patch = patchOf(intent, id);
  return intent.note === undefined ? { kind: "patch", patch } : { kind: "patch", patch, note: intent.note };
}

/** The screen's write path as the edit `edit` (a key stable for the control), named `label` on the chip. */
export function useScreenWrite(edit: string, label?: string): ScreenWrite {
  const write = useWrite(edit, label);
  return {
    run: async (intent, onLanded) => {
      const answer = await write.submit(screenCommand(intent));
      if (answer !== undefined) {
        onLanded?.(isPatchAnswer(answer) ? noticesOf(answer) : []);
      }
      return answer !== undefined;
    },
    pending: write.busy,
    disabled: write.disabled,
    rejection: write.rejection,
    status: write.status,
    dismiss: write.dismiss,
    receipt: write.receipt,
  };
}
