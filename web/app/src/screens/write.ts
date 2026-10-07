// The write path for the screens around a journey (the index, the overview, the route
// screens, entities): one patch through the shell's write path (H5's safe retry, D7's
// notice, nothing sent under version skew), with the rejection kept to show inline (A15).
import { useState } from "react";

import { useSession, useSkew } from "../data/react.ts";
import type { Rejection, WriteIntent } from "../data/writes.ts";

export interface ScreenWrite {
  /** Sends `intent`; true when it landed. */
  run: (intent: WriteIntent) => Promise<boolean>;
  pending: boolean;
  /** Version skew or a write in flight: no new write starts. */
  disabled: boolean;
  /** The last attempt's rejection, until dismissed or a later attempt. */
  rejected: Rejection | undefined;
  dismiss: () => void;
}

export function useScreenWrite(): ScreenWrite {
  const session = useSession();
  const skew = useSkew();
  const [pending, setPending] = useState(false);
  const [rejected, setRejected] = useState<Rejection | undefined>();
  const run = async (intent: WriteIntent): Promise<boolean> => {
    setPending(true);
    const result = await session.write(intent);
    setPending(false);
    setRejected(result.outcome === "rejected" ? result.rejection : undefined);
    return result.outcome === "landed";
  };
  return {
    run,
    pending,
    disabled: pending || skew !== undefined,
    rejected,
    dismiss: () => {
      setRejected(undefined);
    },
  };
}
