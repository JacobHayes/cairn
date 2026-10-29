// The engine's checks of loose values (an id, a name, an address, an email), asked through the
// derive worker as a keyed read (keyed-read.ts): a form shows them as it is typed
// (`useEngineCheck`, react.ts), and a value taken at a moment of its own asks once (`askValues`).
// Both run the one source below, so they have the same policy, answers and failures.
import type { ValueCheck } from "@cairn/wasm";

import type { Deriver } from "./host.ts";
import { DERIVE, readOnce, type ReadEnv, type ReadSource } from "./keyed-read.ts";

/** What the engine says is wrong, by the check's field (a field it has no problem with is absent). */
export type ProblemsByField = Record<string, string>;

/** What an engine check reads: the deriver, and the question put to it. */
export interface EngineAsk {
  deriver: Deriver;
  ask: (deriver: Deriver) => Promise<ProblemsByField>;
}

export const ENGINE_CHECK: ReadSource<EngineAsk, ProblemsByField> = {
  policy: DERIVE,
  read: ({ deriver, ask }) => ask(deriver),
};

/** The engine's problems with `checks`, by field. */
export async function fieldProblems(deriver: Deriver, checks: readonly ValueCheck[]): Promise<ProblemsByField> {
  const problems: ProblemsByField = {};
  for (const problem of await deriver.valueProblems([...checks])) {
    problems[problem.field] = problem.message;
  }
  return problems;
}

/** Asks the engine once about `checks`: for a value taken on a keystroke or a blur, not shown as it changes. */
export function askValues(session: { deriver: Deriver; readEnv: ReadEnv }, checks: readonly ValueCheck[]): Promise<ProblemsByField> {
  const { deriver } = session;
  return readOnce(ENGINE_CHECK, session.readEnv, { of: ["values"], freshness: [JSON.stringify(checks)], args: { deriver, ask: (each) => fieldProblems(each, checks) } });
}
