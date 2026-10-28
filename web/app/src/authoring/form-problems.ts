// A1a, A4, A9, G5: why a node form's values cannot be sent. The rules and their words are the
// engine's (the types the patch reader builds a node from), asked through the derive worker; a
// date rule's own check (`ruleProblem`) and a condition's (`useConditionProblem`) join them here,
// so a form and the proposal editor read one set of problems by field.
import type { NodeFormValues } from "@cairn/wasm";

import { DERIVE, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import { useKeyedRead, useSession } from "../data/react.ts";
import type { Deriver } from "../data/host.ts";
import { useConditionProblem } from "./condition-problem.ts";
import { partsOf, ruleProblem } from "./dates.ts";
import { fieldsOf, type NodeDraft, type FormField } from "./fields.ts";
import type { NodeKind } from "./graph.ts";

/** Why each field cannot be sent, by field. */
export type DraftProblems = Partial<Record<FormField, string>>;

/** The values of the fields a node of `kind` has, as the engine reads them. */
export function formValues(kind: NodeKind, draft: NodeDraft): NodeFormValues {
  const has = new Set<FormField>(fieldsOf(kind, draft.answer_type));
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
  };
}

/** The date rules' own problems, by field. */
function ruleProblems(draft: NodeDraft): DraftProblems {
  const problems: DraftProblems = {};
  for (const which of ["due_by", "not_before"] as const) {
    const rule = draft[which];
    const problem = rule === null ? undefined : ruleProblem(partsOf(rule));
    if (problem !== undefined) {
      problems[which] = problem;
    }
  }
  return problems;
}

async function engineProblems(deriver: Deriver, kind: NodeKind, draft: NodeDraft): Promise<DraftProblems> {
  const problems: DraftProblems = {};
  for (const problem of await deriver.nodeFormProblems(formValues(kind, draft))) {
    problems[problem.field as FormField] = problem.message;
  }
  return problems;
}

/** Every problem that stops `draft` being sent, by field, except a condition's (`useDraftProblems` adds it). */
export async function checkDraft(deriver: Deriver, kind: NodeKind, draft: NodeDraft): Promise<DraftProblems> {
  return { ...(await engineProblems(deriver, kind, draft)), ...ruleProblems(draft) };
}

interface Args {
  deriver: Deriver;
  kind: NodeKind;
  draft: NodeDraft;
}

const PROBLEMS: ReadSource<Args, DraftProblems> = {
  policy: DERIVE,
  read: ({ deriver, kind, draft }) => engineProblems(deriver, kind, draft),
};

/**
 * Every problem that stops `draft` being sent, by field: the engine's for its values, a date
 * rule's, and a condition's. `checked` once the engine has answered for the draft as it is now
 * (nothing is sent before); until then the last answer shows, so a message does not flicker as
 * the author types.
 */
export function useDraftProblems(kind: NodeKind, draft: NodeDraft): { checked: boolean; problems: DraftProblems } {
  const { deriver } = useSession();
  const values = formValues(kind, draft);
  const key: ReadKey<Args> = { of: ["node-form"], freshness: [JSON.stringify(values)], args: { deriver, kind, draft } };
  const { view } = useKeyedRead(PROBLEMS, key);
  const condition = useConditionProblem(draft.relevant_when);
  const held = view.status === "current" || view.status === "refreshing" || view.status === "behind" ? view.value : {};
  const problems: DraftProblems = { ...held, ...ruleProblems(draft), ...(condition.problem === undefined ? {} : { relevant_when: condition.problem }) };
  return { checked: view.status === "current" && condition.checked, problems };
}
