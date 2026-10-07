// A1a: the fields each kind has, and a node form's draft of them. A kind's form offers exactly
// its fields: the shared ones on every kind, then the kind's own (an estimate on work, a
// stage's bounds on a group, a decision's prompt and answer), and on a decision the ones its
// answer type allows (choices on a choice, `fills_role` on an entity or entity list,
// `feeds_milestone` on a date). A saved draft is the one-field mutations of what changed;
// an answer-type change replaces the node whole, the only way the engine takes one (B7:
// a shape change).
import type { Schema } from "@cairn/client";

import type { AnswerType, GraphNode, Mutation, NodeField, NodeKind } from "./graph.ts";
import {
  BODY_BYTES_MAX,
  CHOICE_COUNT_PER_DECISION_MAX,
  OFFSET_DAYS_MAX,
  TITLE_BYTES_MAX,
  WEIGHT_MAX,
  outOfRange,
  overBytes,
} from "./limits.ts";
import { isSlug } from "./keys.ts";

export type Choice = Schema<"Choice">;
export type Condition = Schema<"ConditionResolved">;
export type DateRule = Schema<"DateRule">;

/** What a node form edits: every node field, and a decision's answer type. */
export type FormField = NodeField | "answer_type";

const SHARED: readonly FormField[] = ["id", "title", "description", "weight", "relevant_when", "due_by", "not_before"];

const OWN: Record<NodeKind, readonly FormField[]> = {
  deliverable: ["estimate", "requires_artifact", "placeholder"],
  action: ["estimate", "placeholder"],
  decision: ["prompt", "help", "answer_type", "choices", "fills_role", "feeds_milestone"],
  milestone: ["final", "auto_reach"],
  group: ["opens_at", "closes_at", "gates", "closes"],
};

/** The fields a decision of `answer` takes beyond its prompt, help, and answer type. */
function answerFields(answer: AnswerType): FormField[] {
  switch (answer) {
    case "single_choice":
    case "multi_choice":
      return ["choices"];
    case "entity":
    case "entity_list":
      return ["fills_role"];
    case "date":
      return ["feeds_milestone"];
    case "boolean":
    case "text":
      return [];
  }
}

/** A1a: the fields a node of `kind` (a decision of `answer`) has, in the order its form shows them. */
export function fieldsOf(kind: NodeKind, answer: AnswerType = "boolean"): FormField[] {
  const own = kind === "decision" ? ["prompt", "help", "answer_type", ...answerFields(answer)] : OWN[kind];
  return [...SHARED, ...(own as FormField[])];
}

/** A node form's values as typed: text fields as text, flags as flags, references as keys ("" for none). */
export interface NodeDraft {
  id: string;
  title: string;
  description: string;
  /** "" is the kind's default (A9: 1, a group 0). */
  weight: string;
  /** "" is none. */
  estimate: string;
  requires_artifact: boolean;
  placeholder: boolean;
  final: boolean;
  auto_reach: boolean;
  prompt: string;
  help: string;
  answer_type: AnswerType;
  choices: Choice[];
  fills_role: string;
  feeds_milestone: string;
  opens_at: string;
  closes_at: string;
  gates: boolean;
  closes: boolean;
  relevant_when: Condition | null;
  due_by: DateRule | null;
  not_before: DateRule | null;
}

/** A node's current values as a form's draft. */
export function draftOf(node: GraphNode): NodeDraft {
  return {
    id: node.id,
    title: node.title,
    description: node.description ?? "",
    weight: node.weight == null ? "" : String(node.weight),
    estimate: node.estimate === undefined ? "" : String(node.estimate),
    requires_artifact: node.requires_artifact ?? false,
    placeholder: node.placeholder ?? false,
    final: node.final ?? false,
    auto_reach: node.auto_reach ?? false,
    prompt: node.prompt ?? "",
    help: node.help ?? "",
    answer_type: node.answer_type ?? "boolean",
    choices: node.choices ?? [],
    fills_role: node.fills_role ?? "",
    feeds_milestone: node.feeds_milestone ?? "",
    opens_at: node.opens_at ?? "",
    closes_at: node.closes_at ?? "",
    gates: node.gates ?? true,
    closes: node.closes ?? true,
    relevant_when: node.relevant_when ?? null,
    due_by: node.due_by ?? null,
    not_before: node.not_before ?? null,
  };
}

/** A choice's id. */
export function choiceId(choice: Choice): string {
  return typeof choice === "string" ? choice : choice.id;
}

/** A choice as a person reads it: its label, or its id. */
export function choiceLabel(choice: Choice): string {
  return typeof choice === "string" ? choice : choice.title;
}

const text = (value: string): string | null => (value.trim() === "" ? null : value);
const number = (value: string): number | null => (value.trim() === "" ? null : Number(value));
const key = (value: string): string | null => (value === "" ? null : value);

/** The field's value in the draft, as `set_node_field` writes it (crates/schema `NodeFieldValue`). */
export function valueOf(field: NodeField, draft: NodeDraft): Schema<"NodeFieldValueResolved"> {
  const values: Record<NodeField, unknown> = {
    id: draft.id,
    parent: undefined,
    title: draft.title,
    description: text(draft.description),
    weight: number(draft.weight),
    relevant_when: draft.relevant_when,
    due_by: draft.due_by,
    not_before: draft.not_before,
    estimate: number(draft.estimate),
    placeholder: draft.placeholder,
    requires_artifact: draft.requires_artifact,
    final: draft.final,
    auto_reach: draft.auto_reach,
    opens_at: key(draft.opens_at),
    closes_at: key(draft.closes_at),
    gates: draft.gates,
    closes: draft.closes,
    prompt: draft.prompt,
    help: text(draft.help),
    choices: draft.choices,
    fills_role: key(draft.fills_role),
    feeds_milestone: key(draft.feeds_milestone),
  };
  return { [field]: values[field] } as Schema<"NodeFieldValueResolved">;
}

/** Two values the same as JSON, which is how the engine compares them. */
function same(left: unknown, right: unknown): boolean {
  return JSON.stringify(left ?? null) === JSON.stringify(right ?? null);
}

/** A mutation and the form field that produced it, so a rejection lands at that field (A15). */
export interface FieldMutation {
  field: FormField;
  mutation: Mutation;
}

/** The node as the draft makes it, for an answer-type change (a whole-node replace). */
export function nodeFrom(node: GraphNode, draft: NodeDraft): GraphNode {
  const next: GraphNode = { key: node.key, kind: node.kind, id: draft.id, title: draft.title };
  const set = (field: NodeField) => {
    const value = Object.values(valueOf(field, draft))[0] as unknown;
    if (value !== null && value !== undefined) {
      Object.assign(next, { [field]: value });
    }
  };
  for (const field of fieldsOf(node.kind, draft.answer_type)) {
    if (field !== "answer_type" && field !== "id" && field !== "title") {
      set(field);
    }
  }
  if (node.kind === "decision") {
    next.answer_type = draft.answer_type;
    next.prompt = draft.prompt;
  }
  const kept = { parent: node.parent, requires: node.requires, participations: node.participations, resources: node.resources };
  for (const [name, value] of Object.entries(kept)) {
    if (value !== undefined && value !== null) {
      Object.assign(next, { [name]: value });
    }
  }
  return next;
}

/**
 * What saving `draft` sends for `node`: one `set_node_field` per changed field, or one replace
 * when the answer type changed. With `only`, just the fields its author touched count, so a
 * field changed elsewhere since (H5) is never sent back as it was.
 */
export function changesOf(node: GraphNode, draft: NodeDraft, only?: ReadonlySet<FormField>): FieldMutation[] {
  const touched = (field: FormField) => only === undefined || only.has(field);
  if (node.kind === "decision" && touched("answer_type") && draft.answer_type !== (node.answer_type ?? "boolean")) {
    return [{ field: "answer_type", mutation: { op: "replace_node", node: nodeFrom(node, draft) } }];
  }
  const before = draftOf(node);
  return fieldsOf(node.kind, draft.answer_type).flatMap((field): FieldMutation[] => {
    if (field === "answer_type" || !touched(field)) {
      return [];
    }
    const value = valueOf(field, draft);
    if (same(Object.values(value)[0], Object.values(valueOf(field, before))[0])) {
      return [];
    }
    return [{ field, mutation: { op: "set_node_field", node: node.key, value } }];
  });
}

/** PRACTICES, Explicit limits: why each field of `draft` cannot be sent, by field; empty when all can. */
export function draftProblems(kind: NodeKind, draft: NodeDraft): Partial<Record<FormField, string>> {
  const problems: Partial<Record<FormField, string>> = {};
  const fields = new Set(fieldsOf(kind, draft.answer_type));
  const note = (field: FormField, problem: string | undefined) => {
    if (problem !== undefined && fields.has(field)) {
      problems[field] = problem;
    }
  };
  note("id", isSlug(draft.id) ? undefined : "An id is lowercase letters, digits, '_' and '-', led by a letter or digit, at most 64 bytes.");
  note("title", draft.title.trim() === "" ? "A node needs a title." : overBytes(draft.title, TITLE_BYTES_MAX, "The title"));
  note("description", overBytes(draft.description, BODY_BYTES_MAX, "The description"));
  note("weight", draft.weight === "" ? undefined : outOfRange(Number(draft.weight), WEIGHT_MAX, "A weight"));
  note("estimate", draft.estimate === "" ? undefined : outOfRange(Number(draft.estimate), OFFSET_DAYS_MAX, "An estimate in days"));
  note("prompt", draft.prompt.trim() === "" ? "A decision asks something: it needs a prompt." : overBytes(draft.prompt, BODY_BYTES_MAX, "The prompt"));
  note("help", overBytes(draft.help, BODY_BYTES_MAX, "The help"));
  note("choices", choicesProblem(draft.choices));
  return problems;
}

/** A4 and the choice limit: at least one choice, each an id slug, no id twice. */
export function choicesProblem(choices: readonly Choice[]): string | undefined {
  if (choices.length === 0) {
    return "A choice decision needs at least one choice.";
  }
  if (choices.length > CHOICE_COUNT_PER_DECISION_MAX) {
    return `At most ${String(CHOICE_COUNT_PER_DECISION_MAX)} choices.`;
  }
  const ids = choices.map(choiceId);
  if (ids.some((id) => !isSlug(id))) {
    return "A choice's id is lowercase letters, digits, '_' and '-'.";
  }
  if (new Set(ids).size !== ids.length) {
    return "Two choices share an id.";
  }
  const labels = choices.flatMap((choice) => (typeof choice === "string" ? [] : [choice.title]));
  return labels.map((label) => overBytes(label, TITLE_BYTES_MAX, "A choice's label")).find((problem) => problem !== undefined);
}
