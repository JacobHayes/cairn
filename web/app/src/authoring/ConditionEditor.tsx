// A5: `ConditionEditor` builds a `relevant_when` condition clause by clause. A leaf picks a
// decision outside the node's own subtree, then one of the comparisons its answer type takes,
// then a value of that type (a choice, yes or no, a date, an entity, or text), so a type
// mismatch cannot be entered; `all`, `any`, and `not` compose leaves, and every operator is
// reachable. The condition reads back as a sentence; the limits on depth and clauses are
// checked before it is sent.
import type { Schema } from "@cairn/client";

import { Button, Field } from "../ui/kit.tsx";
import {
  COMBINATORS,
  OPERATOR_WORDS,
  appendAt,
  childrenOfClause,
  combinator,
  conditionWords,
  decisionOf,
  firstValue,
  leaf,
  leavesFor,
  operatorOf,
  referableDecisions,
  replaceAt,
  valueInput,
  valuesOf,
  type Clause,
  type ClausePath,
  type Combinator,
  type Leaf,
  type Value,
  type ValueInput,
} from "./condition.ts";
import { choiceLabel, choiceId } from "./fields.ts";
import type { GraphNode, Tree } from "./graph.ts";
import { Picker } from "./parts.tsx";

type Entity = Schema<"Entity">;

export interface ConditionEditorProps {
  value: Schema<"ConditionResolved"> | null;
  onChange: (next: Schema<"ConditionResolved"> | null) => void;
  tree: Tree;
  /** The node the condition is on; its own subtree's decisions are never offered. */
  node: string | undefined;
  entities: readonly Entity[];
  today: string;
}

interface Context extends ConditionEditorProps {
  decisions: GraphNode[];
  root: Clause;
}

/** A new leaf on `decision`, comparing as its type allows first. */
function newLeaf(decision: GraphNode, context: Pick<Context, "today" | "entities">): Clause {
  const operators = leavesFor(decision.answer_type ?? "boolean");
  const operator = operators.includes("equals") ? "equals" : (operators[0] ?? "answered");
  return leaf(operator, decision.key, [firstValue(valueInput(decision), context.today, context.entities[0]?.key)]);
}

/** A value's words: a choice's label, yes or no, an entity's name. */
export function valueName(tree: Tree, entities: readonly Entity[]) {
  return (value: Value, decision: string): string => {
    if (typeof value === "boolean") {
      return value ? "yes" : "no";
    }
    const choice = (tree.byKey.get(decision)?.choices ?? []).find((each) => choiceId(each) === value);
    if (choice !== undefined) {
      return choiceLabel(choice);
    }
    return entities.find((entity) => entity.key === value)?.name ?? `"${value}"`;
  };
}

/** One value of a leaf, picked as the decision's type allows. */
function ValueControl({ input, value, entities, onChange }: { input: ValueInput; value: Value; entities: readonly Entity[]; onChange: (next: Value) => void }) {
  switch (input.input) {
    case "choice":
      return <Picker aria-label="Value" value={String(value)} options={input.choices.map((choice) => ({ value: choice.id, label: choice.label }))} onChange={(event) => { onChange(event.target.value); }} />;
    case "boolean":
      return (
        <Picker aria-label="Value" value={value === true ? "yes" : "no"} options={[{ value: "yes", label: "yes" }, { value: "no", label: "no" }]} onChange={(event) => { onChange(event.target.value === "yes"); }} />
      );
    case "date":
      return <Field type="date" aria-label="Value" value={String(value)} onChange={(event) => { onChange(event.target.value); }} />;
    case "entity":
      return <Picker aria-label="Value" value={String(value)} options={entities.map((entity) => ({ value: entity.key, label: entity.name }))} onChange={(event) => { onChange(event.target.value); }} />;
    case "text":
      return <Field aria-label="Value" value={String(value)} onChange={(event) => { onChange(event.target.value); }} />;
  }
}

/** A leaf's values: one, or for "is one of" a list with add and remove. */
function LeafValues({ context, clause, path }: { context: Context; clause: Clause; path: ClausePath }) {
  const operator = operatorOf(clause) as Leaf;
  const decisionKey = decisionOf(clause) ?? "";
  const decision = context.tree.byKey.get(decisionKey);
  if (decision === undefined || operator === "answered") {
    return null;
  }
  const input = valueInput(decision);
  const values = valuesOf(clause);
  const set = (next: Value[]) => {
    context.onChange(replaceAt(context.root, path, leaf(operator, decisionKey, next)) ?? null);
  };
  return (
    <>
      {values.map((value, at) => (
        <span key={at} className="row">
          <ValueControl input={input} value={value} entities={context.entities} onChange={(next) => { set(values.map((each, index) => (index === at ? next : each))); }} />
          {operator === "in" && values.length > 1 ? <Button aria-label="Remove this value" onClick={() => { set(values.filter((_, index) => index !== at)); }}>Remove</Button> : null}
        </span>
      ))}
      {operator === "in" ? <Button onClick={() => { set([...values, firstValue(input, context.today, context.entities[0]?.key)]); }}>Add a value</Button> : null}
    </>
  );
}

/** A leaf: the decision, the comparison its type takes, and the value. */
function LeafClause({ context, clause, path }: { context: Context; clause: Clause; path: ClausePath }) {
  const decisionKey = decisionOf(clause) ?? "";
  const decision = context.tree.byKey.get(decisionKey);
  const operators = leavesFor(decision?.answer_type ?? "boolean");
  const replace = (next: Clause | undefined) => {
    context.onChange((next === undefined ? replaceAt(context.root, path, undefined) : replaceAt(context.root, path, next)) ?? null);
  };
  return (
    <span className="row">
      <Picker
        aria-label="Decision"
        value={decisionKey}
        options={context.decisions.map((each) => ({ value: each.key, label: each.title }))}
        onChange={(event) => {
          const picked = context.tree.byKey.get(event.target.value);
          if (picked !== undefined) {
            replace(newLeaf(picked, context));
          }
        }}
      />
      <Picker
        aria-label="Comparison"
        value={operatorOf(clause)}
        options={operators.map((operator) => ({ value: operator, label: OPERATOR_WORDS[operator] }))}
        onChange={(event) => {
          const values = valuesOf(clause);
          replace(leaf(event.target.value as Leaf, decisionKey, values.length > 0 || decision === undefined ? values : [firstValue(valueInput(decision), context.today, context.entities[0]?.key)]));
        }}
      />
      <LeafValues context={context} clause={clause} path={path} />
    </span>
  );
}

/** A clause's own actions: wrap it with another clause, negate it, or remove it. */
function ClauseActions({ context, clause, path }: { context: Context; clause: Clause; path: ClausePath }) {
  const first = context.decisions[0];
  const replace = (next: Clause | undefined) => {
    context.onChange(replaceAt(context.root, path, next) ?? null);
  };
  return (
    <span className="row">
      {first === undefined ? null : <Button onClick={() => { replace(combinator("all", [clause, newLeaf(first, context)])); }}>And another</Button>}
      {first === undefined ? null : <Button onClick={() => { replace(combinator("any", [clause, newLeaf(first, context)])); }}>Or another</Button>}
      {"not" in clause ? null : <Button onClick={() => { replace({ not: clause }); }}>Negate</Button>}
      <Button aria-label="Remove this clause" onClick={() => { replace(undefined); }}>Remove</Button>
    </span>
  );
}

/** A combinator: all of, any of, or not, over its clauses. */
function CombinatorClause({ context, clause, path }: { context: Context; clause: Clause; path: ClausePath }) {
  const operator = operatorOf(clause) as Combinator;
  const children = childrenOfClause(clause);
  const first = context.decisions[0];
  return (
    <>
      <Picker
        aria-label="Combine"
        value={operator}
        options={COMBINATORS.map((each) => ({ value: each, label: OPERATOR_WORDS[each] }))}
        onChange={(event) => { context.onChange(replaceAt(context.root, path, combinator(event.target.value as Combinator, children)) ?? null); }}
      />
      <div className="stack author-clauses">
        {children.map((child, at) => (
          <ClauseView key={at} context={context} clause={child} path={[...path, at]} />
        ))}
      </div>
      {operator === "not" || first === undefined ? null : (
        <Button onClick={() => { context.onChange(appendAt(context.root, path, newLeaf(first, context))); }}>Add a clause</Button>
      )}
    </>
  );
}

function ClauseView({ context, clause, path }: { context: Context; clause: Clause; path: ClausePath }) {
  return (
    <div className="stack author-clause" data-testid="clause" data-path={path.join(".")} data-operator={operatorOf(clause)}>
      {decisionOf(clause) === undefined ? <CombinatorClause context={context} clause={clause} path={path} /> : <LeafClause context={context} clause={clause} path={path} />}
      <ClauseActions context={context} clause={clause} path={path} />
    </div>
  );
}

export function ConditionEditor(props: ConditionEditorProps) {
  const decisions = referableDecisions(props.tree, props.node);
  const first = decisions[0];
  if (props.value === null) {
    return (
      <div className="stack" data-testid="condition-editor">
        <span className="muted">No condition: relevant whenever its ancestors are.</span>
        {first === undefined ? (
          <span className="muted">There is no decision outside this node to make it depend on.</span>
        ) : (
          <span className="row">
            <Button onClick={() => { props.onChange(newLeaf(first, props)); }}>Add a condition</Button>
          </span>
        )}
      </div>
    );
  }
  const context: Context = { ...props, decisions, root: props.value };
  return (
    <div className="stack" data-testid="condition-editor">
      <span className="muted" data-testid="condition-words">
        Relevant when {conditionWords(props.value, props.tree, valueName(props.tree, props.entities))}
      </span>
      <ClauseView context={context} clause={props.value} path={[]} />
    </div>
  );
}
