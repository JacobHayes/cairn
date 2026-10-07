// A5: every operator is reachable, each decision offers exactly the comparisons the engine
// accepts for its answer type (so a type mismatch cannot be entered), and the tree is edited
// clause by clause within its limits.
import type { Schema } from "@cairn/client";
import { beforeAll, describe, expect, it } from "vitest";

import {
  COMBINATORS,
  appendAt,
  combinator,
  conditionProblem,
  decisionsIn,
  firstValue,
  leaf,
  leavesFor,
  referableDecisions,
  replaceAt,
  valueInput,
  type Clause,
  type Leaf,
} from "./condition.ts";
import { applyToDraft, seeded } from "./engine.test-support.ts";
import { treeOf, type AnswerType, type GraphNode } from "./graph.ts";

const ANSWER_TYPES: AnswerType[] = ["boolean", "single_choice", "multi_choice", "text", "date", "entity", "entity_list"];
const LEAVES: Leaf[] = ["equals", "not_equals", "in", "contains", "answered"];

const decision = (answer: AnswerType): GraphNode => ({
  key: `n_${answer}`,
  id: answer.replace("_", "-"),
  kind: "decision",
  title: answer,
  prompt: "?",
  answer_type: answer,
  ...(answer === "single_choice" || answer === "multi_choice" ? { choices: ["first", { id: "second", title: "Second" }] } : {}),
});

const subject: GraphNode = { key: "n_subject", id: "subject", kind: "deliverable", title: "Subject" };

describe("leavesFor (A5)", () => {
  let engine: Awaited<ReturnType<typeof seeded>>["engine"];
  let deployment: Schema<"Deployment">;
  beforeAll(async () => {
    const host = await seeded();
    engine = host.engine;
    deployment = host.deployment();
  });

  it("reaches every operator across the answer types", () => {
    const reached = new Set([...ANSWER_TYPES.flatMap((answer) => leavesFor(answer)), ...COMBINATORS]);
    expect([...reached].sort()).toEqual(["all", "answered", "any", "contains", "equals", "in", "not", "not_equals"]);
  });

  it.each(ANSWER_TYPES)("a %s decision offers exactly the comparisons the engine accepts", (answer) => {
    const asked = decision(answer);
    const value = firstValue(valueInput(asked), "2026-10-06", "e_lead");
    for (const operator of LEAVES) {
      const condition = leaf(operator, asked.key, [value]);
      const outcome = applyToDraft(engine, deployment, { nodes: [asked, subject] }, [{ op: "set_node_field", node: subject.key, value: { relevant_when: condition } }]);
      expect([operator, outcome.accepted]).toEqual([operator, leavesFor(answer).includes(operator)]);
    }
  });

  it("picks a value of the decision's type to start", () => {
    expect(firstValue(valueInput(decision("single_choice")), "2026-10-06", undefined)).toBe("first");
    expect(firstValue(valueInput(decision("boolean")), "2026-10-06", undefined)).toBe(true);
    expect(firstValue(valueInput(decision("date")), "2026-10-06", undefined)).toBe("2026-10-06");
  });
});

describe("editing the tree", () => {
  const yes = leaf("equals", "n_boolean", [true]);
  const answered = leaf("answered", "n_text", []);

  it("appends to a combinator and replaces or removes a clause by its path", () => {
    const root = appendAt(combinator("all", [yes]), [], answered);
    expect(root).toEqual({ all: [yes, answered] });
    expect(replaceAt(root, [1], leaf("answered", "n_date", []))).toEqual({ all: [yes, { answered: "n_date" }] });
    expect(replaceAt(root, [0], undefined)).toEqual({ all: [answered] });
    expect(replaceAt({ not: yes }, [0], undefined)).toBeUndefined();
  });

  it("names every decision it reads", () => {
    expect(decisionsIn({ any: [yes, { not: answered }] })).toEqual(["n_boolean", "n_text"]);
  });

  it("is refused before sending past its limits or with an empty combinator", () => {
    let deep: Clause = yes;
    for (let level = 0; level < 8; level += 1) {
      deep = { not: deep };
    }
    expect(conditionProblem(deep)).toBeDefined();
    expect(conditionProblem({ all: Array.from({ length: 16 }, () => yes) })).toBeDefined();
    expect(conditionProblem({ any: [] })).toBeDefined();
    expect(conditionProblem({ all: [yes, { not: answered }] })).toBeUndefined();
  });

  it("never offers a decision inside the node's own subtree (Invariants)", () => {
    const group: GraphNode = { key: "n_group", id: "group", kind: "group", title: "Group" };
    const inside = { ...decision("boolean"), parent: "n_group" };
    const outside = decision("text");
    const tree = treeOf({ nodes: [group, inside, outside] });
    expect(referableDecisions(tree, "n_group").map((node) => node.key)).toEqual([outside.key]);
    expect(referableDecisions(tree, undefined).map((node) => node.key).sort()).toEqual([inside.key, outside.key].sort());
  });
});
