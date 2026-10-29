// A1a: the fields each kind has, and a node form's draft of them. A kind's form offers exactly
// its fields: the shared ones on every kind, then the kind's own (an estimate on work, a
// stage's bounds on a group, a decision's prompt and answer), and on a decision the ones its
// answer type allows (choices on a choice, `fills_role` on an identity or identity list,
// `feeds_milestone` on a date). A saved draft is the one-field mutations of what changed;
// an answer-type change replaces the node whole, the only way the engine takes one (B7:
// a shape change).
import { ANSWER_TYPE_FIELDS, FLAG_DEFAULTS, KIND_FIELDS, NODE_FIELDS, type KindFieldName } from "@cairn/client";
import type { Schema } from "@cairn/client";

import type { Choice } from "../status/answer.ts";
import { partsOf, ruleOf, type RuleParts } from "./dates.ts";
import type { AnswerType, GraphNode, Mutation, NodeField, NodeKind } from "./graph.ts";

export type Condition = Schema<"ConditionResolved">;
export type DateRule = Schema<"DateRule">;

/** What a node form edits: every node field, and a decision's answer type. */
export type FormField = NodeField | "answer_type";

const kindFieldNames: ReadonlySet<string> = new Set(Object.values(KIND_FIELDS).flat());

/** The fields every kind has: the node fields that no kind alone has, except the move (`parent`), which has its own control. */
const SHARED: readonly FormField[] = NODE_FIELDS.filter((field) => !kindFieldNames.has(field) && field !== "parent");

/** A1a: the fields a node of `kind` (a decision of `answer`) has, in the order its form shows them (the engine's table). */
export function fieldsOf(kind: NodeKind, answer: AnswerType = "boolean"): FormField[] {
  const own: readonly KindFieldName[] = kind === "decision" ? ANSWER_TYPE_FIELDS[answer] : KIND_FIELDS[kind];
  return [...SHARED, ...own];
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
  requires_note: boolean;
  requires_checklist: boolean;
  checklist: Schema<"ChecklistItem">[];
  placeholder: boolean;
  final: boolean;
  auto_done: boolean;
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
  due_by: RuleParts | null;
  not_before: RuleParts | null;
}

/** A node's current values as a form's draft. */
export function draftOf(node: GraphNode): NodeDraft {
  return {
    id: node.id,
    title: node.title,
    description: node.description ?? "",
    weight: node.weight == null ? "" : String(node.weight),
    estimate: node.estimate === undefined ? "" : String(node.estimate),
    requires_artifact: node.requires_artifact ?? FLAG_DEFAULTS.requires_artifact,
    requires_note: node.requires_note ?? FLAG_DEFAULTS.requires_note,
    requires_checklist: node.requires_checklist ?? FLAG_DEFAULTS.requires_checklist,
    checklist: node.checklist ?? [],
    placeholder: node.placeholder ?? FLAG_DEFAULTS.placeholder,
    final: node.final ?? FLAG_DEFAULTS.final,
    auto_done: node.auto_done ?? FLAG_DEFAULTS.auto_done,
    prompt: node.prompt ?? "",
    help: node.help ?? "",
    answer_type: node.answer_type ?? "boolean",
    choices: node.choices ?? [],
    fills_role: node.fills_role ?? "",
    feeds_milestone: node.feeds_milestone ?? "",
    opens_at: node.opens_at ?? "",
    closes_at: node.closes_at ?? "",
    gates: node.gates ?? FLAG_DEFAULTS.gates,
    closes: node.closes ?? FLAG_DEFAULTS.closes,
    relevant_when: node.relevant_when ?? null,
    due_by: node.due_by == null ? null : partsOf(node.due_by),
    not_before: node.not_before == null ? null : partsOf(node.not_before),
  };
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
    due_by: draft.due_by === null ? null : ruleOf(draft.due_by),
    not_before: draft.not_before === null ? null : ruleOf(draft.not_before),
    estimate: number(draft.estimate),
    placeholder: draft.placeholder,
    requires_artifact: draft.requires_artifact,
    requires_note: draft.requires_note,
    requires_checklist: draft.requires_checklist,
    checklist: draft.checklist,
    final: draft.final,
    auto_done: draft.auto_done,
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
