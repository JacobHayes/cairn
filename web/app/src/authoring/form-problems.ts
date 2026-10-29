// A1a, A4, A9, G5: why a form's values cannot be sent. The rules and their words are the
// engine's (the types the patch reader builds the values from), asked through the derive worker:
// a node form's values (`nodeFormProblems`, with its date rules), and the loose values of the other
// forms (`valueProblems`: an id, a name, an address). A condition's own check
// (`useConditionProblem`) joins a node form's, so a form and the proposal editor read one set of
// problems by field. A check that cannot be made is a `failure` with a way to `retry`; nothing is
// sendable until the engine has answered.
import type { CountLimit, NodeFormValues, ValueCheck } from "@cairn/wasm";

import { fieldProblems, TYPED_CHECK, type EngineAsk, type ProblemsByField } from "../data/checks.ts";
import { useEngineCheck, type Checked } from "../data/react.ts";
import type { Deriver } from "../data/host.ts";
import type { ReadSource } from "../data/keyed-read.ts";
import { useConditionProblem } from "./condition-problem.ts";
import { fieldsOf, type NodeDraft, type FormField } from "./fields.ts";
import type { NodeKind } from "./graph.ts";

/** Why each field cannot be sent, by field. */
export type DraftProblems = Partial<Record<FormField, string>>;

/** The values of the fields a node of `kind` has, as the engine reads them. */
export function formValues(kind: NodeKind, draft: NodeDraft): NodeFormValues {
  const has = new Set<FormField>(fieldsOf(kind, draft.answer_type));
  const rule = (which: "due_by" | "not_before"): NodeFormValues => {
    const value = draft[which];
    if (value === null) {
      return {};
    }
    return { [which]: { sources: value.sources, offset: value.offset.trim() === "" ? null : Number(value.offset) } };
  };
  return {
    ...(has.has("id") ? { id: draft.id } : {}),
    ...(has.has("title") ? { title: draft.title } : {}),
    ...(has.has("description") ? { description: draft.description } : {}),
    ...(has.has("weight") ? { weight: draft.weight } : {}),
    ...(has.has("estimate") ? { estimate: draft.estimate } : {}),
    ...(has.has("prompt") ? { prompt: draft.prompt } : {}),
    ...(has.has("help") ? { help: draft.help } : {}),
    ...(has.has("choices") ? { choices: draft.choices } : {}),
    ...(has.has("checklist") ? { checklist: draft.checklist } : {}),
    ...rule("due_by"),
    ...rule("not_before"),
  };
}

async function nodeProblems(deriver: Deriver, values: NodeFormValues): Promise<ProblemsByField> {
  const problems: ProblemsByField = {};
  for (const problem of await deriver.nodeFormProblems(values)) {
    problems[problem.field] = problem.message;
  }
  return problems;
}

/**
 * Every problem that stops `draft` being sent, by field: the engine's for its values (with its
 * date rules), and a condition's.
 */
export function useDraftProblems(kind: NodeKind, draft: NodeDraft): Checked<DraftProblems> {
  const values = formValues(kind, draft);
  const engine = useEngineCheck("node-form", values, (deriver) => nodeProblems(deriver, values), TYPED_CHECK);
  const condition = useConditionProblem(draft.relevant_when);
  const problems: DraftProblems = { ...engine.problems, ...(condition.problem === undefined ? {} : { relevant_when: condition.problem }) };
  return {
    checked: engine.checked && condition.checked,
    problems,
    failure: engine.failure ?? condition.failure,
    retry: () => {
      engine.retry();
      condition.retry();
    },
  };
}

/** Why each of `checks` cannot be sent, by the check's `field`: the engine's, from the types the patch reader reads the same values with. */
export function useValueProblems(checks: readonly ValueCheck[], source: ReadSource<EngineAsk, ProblemsByField> = TYPED_CHECK): Checked<Record<string, string | undefined>> {
  return useEngineCheck("values", checks, (deriver) => fieldProblems(deriver, checks), source);
}

/** The check that `count` things (what the graph or list would hold) are within `limit`; `what` completes "At most N ...". */
export function countCheck(field: string, limit: CountLimit, what: string, count: number): ValueCheck {
  return { field, kind: { count: { limit } }, what, value: String(count) };
}
