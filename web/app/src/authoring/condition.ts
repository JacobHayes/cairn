// A5: a `relevant_when` condition as a tree an editor builds clause by clause (ARCHITECTURE,
// Engine > Conditions: a structured predicate tree, a fixed operator set, no parser). Every
// operator is reachable: `all`, `any`, and `not` compose; the leaves compare a decision's
// answer. Which leaves a decision offers follows its answer type exactly as the engine checks
// it (crates/engine validate/references.rs, `Comparable::fits`), and a value is picked from
// what that type holds (a choice, yes or no, a date, an entity), so a type mismatch cannot be
// entered. A condition cannot name a decision in its own node's subtree (Invariants), so the
// editor never offers one. Rendering to a sentence is for display and never parsed back.
import type { AnswerType, GraphNode, Tree } from "./graph.ts";
import { descendantsOf, nodesByPath, titleIn } from "./graph.ts";
import { choiceId, choiceLabel, type Condition } from "./fields.ts";
import { CONDITION_CLAUSE_COUNT_MAX, CONDITION_DEPTH_MAX } from "./limits.ts";

export type Clause = Condition;
export type Value = boolean | string;

/** A leaf operator: it compares one decision's answer. */
export type Leaf = "equals" | "not_equals" | "in" | "contains" | "answered";
/** A combinator: it composes clauses. */
export type Combinator = "all" | "any" | "not";
export type Operator = Leaf | Combinator;

export const COMBINATORS: readonly Combinator[] = ["all", "any", "not"];

/** A5: the leaves each answer type can be compared by (the engine's `equal_fits` and `member_fits`). */
const LEAVES: Record<AnswerType, readonly Leaf[]> = {
  boolean: ["equals", "not_equals", "in", "answered"],
  single_choice: ["equals", "not_equals", "in", "answered"],
  text: ["equals", "not_equals", "in", "contains", "answered"],
  date: ["equals", "not_equals", "in", "answered"],
  entity: ["equals", "not_equals", "in", "answered"],
  multi_choice: ["contains", "answered"],
  entity_list: ["contains", "answered"],
};

/** A5: the leaf operators a decision of `answer` offers. */
export function leavesFor(answer: AnswerType): readonly Leaf[] {
  return LEAVES[answer];
}

/** The words for an operator, as an editor's select shows them. */
export const OPERATOR_WORDS: Record<Operator, string> = {
  equals: "equals",
  not_equals: "does not equal",
  in: "is one of",
  contains: "contains",
  answered: "is answered",
  all: "all of",
  any: "any of",
  not: "not",
};

/** How a decision's values are picked: from its choices, yes or no, a date, an entity, or typed. */
export type ValueInput =
  | { input: "choice"; choices: { id: string; label: string }[] }
  | { input: "boolean" }
  | { input: "date" }
  | { input: "entity" }
  | { input: "text" };

/** A5: what a decision's clause compares against, by its answer type. */
export function valueInput(decision: GraphNode): ValueInput {
  switch (decision.answer_type ?? "boolean") {
    case "single_choice":
    case "multi_choice":
      return { input: "choice", choices: (decision.choices ?? []).map((choice) => ({ id: choiceId(choice), label: choiceLabel(choice) })) };
    case "boolean":
      return { input: "boolean" };
    case "date":
      return { input: "date" };
    case "entity":
    case "entity_list":
      return { input: "entity" };
    case "text":
      return { input: "text" };
  }
}

/** A value the input offers first, so a new clause is complete and well typed from the start. */
export function firstValue(input: ValueInput, today: string, entity: string | undefined): Value {
  switch (input.input) {
    case "choice":
      return input.choices[0]?.id ?? "";
    case "boolean":
      return true;
    case "date":
      return today;
    case "entity":
      return entity ?? "";
    case "text":
      return "";
  }
}

/** The decisions `node`'s condition may name: every decision outside its own subtree (Invariants). */
export function referableDecisions(tree: Tree, node: string | undefined): GraphNode[] {
  const own = new Set(node === undefined ? [] : [node, ...descendantsOf(tree, node).map((each) => each.key)]);
  return nodesByPath(tree, "decision").filter((decision) => !own.has(decision.key));
}

/** The operator a clause uses. */
export function operatorOf(clause: Clause): Operator {
  return Object.keys(clause)[0] as Operator;
}

/** A clause's children: a combinator's operands, none for a leaf. */
export function childrenOfClause(clause: Clause): Clause[] {
  if ("all" in clause) {
    return clause.all;
  }
  if ("any" in clause) {
    return clause.any;
  }
  if ("not" in clause) {
    return [clause.not];
  }
  return [];
}

/** The decision a leaf compares, undefined for a combinator. */
export function decisionOf(clause: Clause): string | undefined {
  if ("answered" in clause) {
    return clause.answered;
  }
  if ("equals" in clause) {
    return clause.equals.decision;
  }
  if ("not_equals" in clause) {
    return clause.not_equals.decision;
  }
  if ("contains" in clause) {
    return clause.contains.decision;
  }
  if ("in" in clause) {
    return clause.in.decision;
  }
  return undefined;
}

/** The values a leaf compares with (none for `answered` and combinators). */
export function valuesOf(clause: Clause): Value[] {
  if ("equals" in clause) {
    return [clause.equals.value];
  }
  if ("not_equals" in clause) {
    return [clause.not_equals.value];
  }
  if ("contains" in clause) {
    return [clause.contains.value];
  }
  if ("in" in clause) {
    return clause.in.values;
  }
  return [];
}

/** A leaf of `operator` on `decision` comparing `values` (the first for a single-value leaf). */
export function leaf(operator: Leaf, decision: string, values: readonly Value[]): Clause {
  const value = values[0] ?? "";
  switch (operator) {
    case "answered":
      return { answered: decision };
    case "equals":
      return { equals: { decision, value } };
    case "not_equals":
      return { not_equals: { decision, value } };
    case "contains":
      return { contains: { decision, value } };
    case "in":
      return { in: { decision, values: values.length === 0 ? [value] : [...values] } };
  }
}

/** A combinator over `children` (`not` takes the first). */
export function combinator(operator: Combinator, children: readonly Clause[]): Clause {
  switch (operator) {
    case "all":
      return { all: [...children] };
    case "any":
      return { any: [...children] };
    case "not":
      return { not: children[0] ?? { all: [] } };
  }
}

/** A path into a condition: the child index at each level. */
export type ClausePath = readonly number[];

/** The clause at `path`. */
export function clauseAt(root: Clause, path: ClausePath): Clause | undefined {
  let clause: Clause | undefined = root;
  for (const index of path) {
    clause = clause === undefined ? undefined : childrenOfClause(clause)[index];
  }
  return clause;
}

/** `root` with the clause at `path` replaced by `next`, or removed when `next` is undefined. */
export function replaceAt(root: Clause, path: ClausePath, next: Clause | undefined): Clause | undefined {
  const [head, ...rest] = path;
  if (head === undefined) {
    return next;
  }
  const children = childrenOfClause(root);
  const child = children[head];
  if (child === undefined) {
    return root;
  }
  const replaced = replaceAt(child, rest, next);
  const kept = children.flatMap((each, index) => (index === head ? (replaced === undefined ? [] : [replaced]) : [each]));
  if ("not" in root) {
    return kept[0] === undefined ? undefined : { not: kept[0] };
  }
  return combinator(operatorOf(root) as Combinator, kept);
}

/** `root` with `clause` added as the last operand of the combinator at `path`. */
export function appendAt(root: Clause, path: ClausePath, clause: Clause): Clause {
  const parent = clauseAt(root, path);
  if (parent === undefined || !("all" in parent || "any" in parent)) {
    return root;
  }
  return replaceAt(root, path, combinator(operatorOf(parent) as Combinator, [...childrenOfClause(parent), clause])) ?? root;
}

/** The condition's depth (a leaf is 1) and clause count, which the limits bound. */
export function sizeOf(clause: Clause): { depth: number; clauses: number } {
  const children = childrenOfClause(clause).map(sizeOf);
  return {
    depth: 1 + Math.max(0, ...children.map((child) => child.depth)),
    clauses: 1 + children.reduce((sum, child) => sum + child.clauses, 0),
  };
}

/** Why the condition cannot be sent: past a limit, an empty combinator, or a leaf with nothing to compare. */
export function conditionProblem(clause: Clause): string | undefined {
  const { depth, clauses } = sizeOf(clause);
  if (depth > CONDITION_DEPTH_MAX) {
    return `A condition is at most ${String(CONDITION_DEPTH_MAX)} levels deep.`;
  }
  if (clauses > CONDITION_CLAUSE_COUNT_MAX) {
    return `A condition has at most ${String(CONDITION_CLAUSE_COUNT_MAX)} clauses.`;
  }
  const empty = (each: Clause): boolean =>
    (("all" in each || "any" in each) && childrenOfClause(each).length === 0) ||
    ("in" in each && each.in.values.length === 0) ||
    childrenOfClause(each).some(empty);
  return empty(clause) ? "Every 'all of', 'any of', and 'is one of' needs something in it." : undefined;
}

/** Every decision a condition names. */
export function decisionsIn(clause: Clause): string[] {
  const own = decisionOf(clause);
  return [...(own === undefined ? [] : [own]), ...childrenOfClause(clause).flatMap(decisionsIn)];
}

/** A condition as a sentence, for display only (ARCHITECTURE: rendered, never parsed back). */
export function conditionWords(clause: Clause, tree: Tree, name: (value: Value, decision: string) => string): string {
  const operator = operatorOf(clause);
  const decision = decisionOf(clause);
  if (decision !== undefined) {
    const values = valuesOf(clause).map((value) => name(value, decision));
    const subject = `"${titleIn(tree, decision)}"`;
    return operator === "answered" ? `${subject} is answered` : `${subject} ${OPERATOR_WORDS[operator]} ${values.join(", ")}`;
  }
  const parts = childrenOfClause(clause).map((child) => conditionWords(child, tree, name));
  if (operator === "not") {
    return `not (${parts.join("")})`;
  }
  return parts.length === 0 ? "(nothing)" : `(${parts.join(operator === "all" ? " and " : " or ")})`;
}
