// Proposal writes from a screen (I6): each names a fresh patch id (H5), is refused before
// anything is sent under version skew (ARCHITECTURE, Web UI), notes what an apply caused (D7),
// and keeps its rejection for the control that sent it. Nothing retries on its own: a stale
// proposal is refreshed and reviewed again by a person, never rebased behind their back.
import type { HttpFailure } from "@cairn/client";
import { useState } from "react";

import { useSession, useSkew } from "../data/react.ts";
import { consequenceLines } from "../data/notices.ts";
import type { PatchAnswer, ProposalHost, ProposalWritten, Rejection } from "../data/proposals.ts";
import { newPatchId } from "../data/writes.ts";

/** What went wrong with the last write: the rejection, or why there was no answer. */
export type WriteProblem = { rejection: Rejection } | { failure: HttpFailure };

export interface ProposalWrite {
  /** Runs `call` with `patchId`, a fresh one by default; its answer when it was answered. */
  run: <A>(call: (proposals: ProposalHost, patchId: string) => Promise<ProposalWritten<A>>, patchId?: string) => Promise<A | undefined>;
  pending: boolean;
  disabled: boolean;
  problem: WriteProblem | undefined;
  dismiss: () => void;
}

/** True for an apply's answer, which notes what it caused (D7). */
function isPatchAnswer(answer: unknown): answer is PatchAnswer {
  return typeof answer === "object" && answer !== null && "outcome" in answer && (answer.outcome === "applied" || answer.outcome === "already_applied");
}

export function useProposalWrite(): ProposalWrite {
  const session = useSession();
  const skew = useSkew();
  const [pending, setPending] = useState(false);
  const [problem, setProblem] = useState<WriteProblem | undefined>();
  const run = async <A>(call: (proposals: ProposalHost, patchId: string) => Promise<ProposalWritten<A>>, patchId: string = newPatchId()): Promise<A | undefined> => {
    if (skew !== undefined || session.skew.latched) {
      return undefined;
    }
    setPending(true);
    setProblem(undefined);
    const written = await call(session.host.proposals, patchId);
    setPending(false);
    switch (written.outcome) {
      case "answered":
        if (isPatchAnswer(written.answer)) {
          session.notices.add({ tone: "saved", title: "Applied", lines: consequenceLines(written.answer) });
        }
        return written.answer;
      case "rejected":
        setProblem({ rejection: written.rejection });
        return undefined;
      case "failed":
        setProblem({ failure: written.error });
        return undefined;
    }
  };
  return { run, pending, disabled: pending || skew !== undefined, problem, dismiss: () => { setProblem(undefined); } };
}
