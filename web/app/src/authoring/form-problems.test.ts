// A1a, A4, A9, G5: the form's values are read by the engine through the types the patch reader
// builds a node from, so a value the form is told it can send is one the reader takes, and a
// value it is told it cannot is one the reader refuses. These run the real engine.
import { beforeAll, describe, expect, it } from "vitest";
import type { FieldProblem } from "@cairn/wasm";

import { applyToDraft, seeded } from "./engine.test-support.ts";
import { changesOf, draftOf, type NodeDraft } from "./fields.ts";
import { formValues } from "./form-problems.ts";
import type { GraphNode } from "./graph.ts";

const decision: GraphNode = { key: "n_ask", id: "ask", kind: "decision", title: "Ask", prompt: "Which?", answer_type: "single_choice", choices: ["one"] };
const deliverable: GraphNode = { key: "n_work", id: "work", kind: "deliverable", title: "Work" };

let host: Awaited<ReturnType<typeof seeded>>;
beforeAll(async () => {
  host = await seeded();
});

const problemsOf = (node: GraphNode, draft: NodeDraft): FieldProblem[] => JSON.parse(host.engine.nodeFormProblemsText(formValues(node.kind, draft))) as FieldProblem[];

/** Whether the patch reader takes what saving `draft` sends. */
function reads(node: GraphNode, draft: NodeDraft): boolean {
  const mutations = changesOf(node, draft).map((change) => change.mutation);
  try {
    applyToDraft(host.engine, host.deployment(), { nodes: [node] }, mutations);
    return true;
  } catch {
    return false;
  }
}

const item = (at: number, title = "Item") => ({ id: `i${String(at)}`, title });
const many = <T>(count: number, make: (at: number) => T): T[] => Array.from({ length: count }, (_, at) => make(at));

const cases: [string, GraphNode, Partial<NodeDraft>, string | undefined][] = [
  ["an id that is not a slug", deliverable, { id: "Not a slug" }, "id"],
  ["a blank title", deliverable, { title: "  " }, "title"],
  ["a title with a line break", deliverable, { title: "two\nlines" }, "title"],
  ["a title past its byte limit", deliverable, { title: "t".repeat(257) }, "title"],
  ["a title within it", deliverable, { title: "t".repeat(256) }, undefined],
  ["a weight past the limit", deliverable, { weight: "1001" }, "weight"],
  ["a weight at the limit", deliverable, { weight: "1000" }, undefined],
  ["a fractional estimate", deliverable, { estimate: "1.5" }, "estimate"],
  ["a whole number the way the box writes it", deliverable, { estimate: "1e2", weight: "1.0" }, undefined],
  ["an estimate past a year", deliverable, { estimate: "366" }, "estimate"],
  ["a blank prompt", decision, { prompt: " " }, "prompt"],
  ["no choices", decision, { choices: [] }, "choices"],
  ["a choice id twice", decision, { choices: ["a", "a"] }, "choices"],
  ["a choice id that is not a slug", decision, { choices: ["Up"] }, "choices"],
  ["a choice label that is blank", decision, { choices: [{ id: "a", title: "" }] }, "choices"],
  ["thirty-three choices", decision, { choices: many(33, (at) => `c${String(at)}`) }, "choices"],
  ["thirty-two choices", decision, { choices: many(32, (at) => `c${String(at)}`) }, undefined],
  ["fifty-one checklist items", deliverable, { checklist: many(51, item) }, "checklist"],
  ["fifty checklist items", deliverable, { checklist: many(50, item) }, undefined],
  ["a checklist item with a blank title", deliverable, { checklist: [item(1), item(2, " ")] }, "checklist"],
];

describe("nodeFormProblems", () => {
  it.each(cases)("%s", (_, node, change, field) => {
    const draft = { ...draftOf(node), ...change };
    expect(problemsOf(node, draft).map((problem) => problem.field)).toEqual(field === undefined ? [] : [field]);
    if (field !== undefined) {
      expect(problemsOf(node, draft)[0]?.message).not.toBe("");
    }
    // A field the node has no use for is never checked; a clean draft is always one the reader takes.
    expect(reads(node, draft)).toBe(field === undefined);
  });

  it("checks only the fields a kind has", () => {
    expect(problemsOf(deliverable, { ...draftOf(deliverable), prompt: "" })).toEqual([]);
    expect(Object.keys(formValues("deliverable", draftOf(deliverable)))).not.toContain("prompt");
  });

  it("says what is wrong in words, at the field", () => {
    expect(problemsOf(deliverable, { ...draftOf(deliverable), title: "" })).toEqual([{ field: "title", message: "The title cannot be blank." }]);
    expect(problemsOf(deliverable, { ...draftOf(deliverable), title: "t".repeat(300) })).toEqual([{ field: "title", message: "The title is 300 bytes; at most 256. Shorten it." }]);
  });
});
