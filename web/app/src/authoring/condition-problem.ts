// A5: what keeps a condition the editor holds from being sent: past a limit, or an empty "all of"
// or "any of". The rule is the engine's (`Condition::new`), asked through the derive worker; the
// words and the maxima they name are the app's, the maxima from the generated limits.
import { CONDITION_CLAUSE_COUNT_MAX, CONDITION_DEPTH_MAX } from "@cairn/client";
import type { ConditionProblem } from "@cairn/wasm";

import { DERIVE, type ReadError, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { Deriver } from "../data/host.ts";
import type { Condition } from "./fields.ts";

/** The problem in words. */
export function conditionProblemWords(problem: ConditionProblem): string {
  if (problem === "empty_combination") {
    return "Every 'all of' and 'any of' needs something in it.";
  }
  if ("unreadable" in problem) {
    return `This condition cannot be sent: ${problem.unreadable.reason}`;
  }
  return problem.too_large.limit === "condition_depth_max"
    ? `A condition is at most ${String(CONDITION_DEPTH_MAX)} levels deep.`
    : `A condition compares at most ${String(CONDITION_CLAUSE_COUNT_MAX)} answers.`;
}

interface Args {
  deriver: Deriver;
  clause: Condition;
}

const PROBLEM: ReadSource<Args, string | undefined> = {
  policy: DERIVE,
  read: async ({ deriver, clause }) => {
    const problem = await deriver.conditionProblem(clause);
    return problem === undefined ? undefined : conditionProblemWords(problem);
  },
};

/**
 * What the engine says of `clause`: `checked` once it has answered (a form waits for it before it
 * is sent; no clause has nothing to check), and the problem in words, if there is one.
 */
export function useConditionProblem(clause: Condition | null): { checked: boolean; problem: string | undefined; failure: ReadError | undefined; retry: () => void } {
  const { deriver } = useSession();
  const key: ReadKey<Args> | undefined = clause === null ? undefined : { of: ["condition"], freshness: [JSON.stringify(clause)], args: { deriver, clause } };
  const { view, refresh } = useKeyedRead(PROBLEM, key);
  if (clause === null) {
    return { checked: true, problem: undefined, failure: undefined, retry: refresh };
  }
  const failure = view.status === "failed" || view.status === "behind" ? view.error : undefined;
  return view.status === "current" ? { checked: true, problem: view.value, failure, retry: refresh } : { checked: false, problem: undefined, failure, retry: refresh };
}
