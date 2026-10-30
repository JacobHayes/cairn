// A1a: a kind's form offers exactly the fields the engine lets that kind have, a decision's by
// its answer type; a saved draft sends one mutation per changed field, and an answer-type
// change replaces the node whole; limits are caught before anything is sent.
import type { Schema } from "@cairn/client";
import { beforeAll, describe, expect, it } from "vitest";

import { applyToDraft, seeded } from "./engine.test-support.ts";
import { changesOf, draftOf, fieldsOf, type NodeDraft } from "./fields.ts";
import type { AnswerType, Graph, GraphNode, NodeField, NodeKind } from "./graph.ts";

type Deployment = Schema<"Deployment">;

/** A well-formed value for every node field, so only whether the kind has it decides. */
const SAMPLE: Record<NodeField, unknown> = {
  id: "renamed",
  parent: null,
  title: "Renamed",
  description: null,
  weight: null,
  relevant_when: null,
  due_by: null,
  not_before: null,
  estimate: null,
  placeholder: false,
  requires_artifact: false,
  requires_note: false,
  checklist: [],
  requires_checklist: false,
  final: false,
  auto_done: false,
  prompt: "Asked?",
  help: null,
  choices: ["one"],
  fills_role: null,
  feeds_milestone: null,
};

const node = (kind: NodeKind, answer?: AnswerType): GraphNode => ({
  key: "n_subject",
  id: "subject",
  kind,
  title: "Subject",
  ...(kind === "decision" ? { prompt: "Which?", answer_type: answer ?? "boolean", ...(answer === "single_choice" || answer === "multi_choice" ? { choices: ["one"] } : {}) } : {}),
});

const graphOf = (...nodes: GraphNode[]): Graph => ({ nodes });

const cases: [NodeKind, AnswerType | undefined][] = [
  ["group", undefined],
  ["deliverable", undefined],
  ["action", undefined],
  ["milestone", undefined],
  ...(["boolean", "single_choice", "multi_choice", "text", "date", "identity", "identity_list"] as const).map((answer): [NodeKind, AnswerType] => ["decision", answer]),
];

describe("fieldsOf (A1a)", () => {
  let deployment: Deployment;
  let engine: Awaited<ReturnType<typeof seeded>>["engine"];
  beforeAll(async () => {
    const host = await seeded();
    deployment = host.deployment();
    engine = host.engine;
  });

  it.each(cases)("a %s (%s) form offers exactly the fields the engine lets it have", (kind, answer) => {
    const subject = node(kind, answer);
    const offered = new Set<string>(fieldsOf(kind, answer));
    for (const [field, value] of Object.entries(SAMPLE)) {
      const outcome = applyToDraft(engine, deployment, graphOf(subject), [{ op: "set_node_field", node: subject.key, value: { [field]: value } as Schema<"NodeFieldValueResolved"> }]);
      const has = outcome.accepted || !outcome.codes.includes("field_not_on_kind");
      // `parent` is every kind's, moved by the move control rather than the form.
      expect([field, has]).toEqual([field, offered.has(field) || field === "parent"]);
    }
  });
});

describe("changesOf", () => {
  const deliverable: GraphNode = { key: "n_report", id: "report", kind: "deliverable", title: "Report", estimate: 3 };

  it("sends nothing for an unchanged draft", () => {
    expect(changesOf(deliverable, draftOf(deliverable))).toEqual([]);
  });

  it("sends one set_node_field per changed field, each naming its field", () => {
    const draft: NodeDraft = { ...draftOf(deliverable), title: "Final report", estimate: "", requires_artifact: true };
    expect(changesOf(deliverable, draft)).toEqual([
      { field: "title", mutation: { op: "set_node_field", node: "n_report", value: { title: "Final report" } } },
      { field: "estimate", mutation: { op: "set_node_field", node: "n_report", value: { estimate: null } } },
      { field: "requires_artifact", mutation: { op: "set_node_field", node: "n_report", value: { requires_artifact: true } } },
    ]);
  });

  it("sends only the fields its author touched, so one changed elsewhere since is not sent back (H5)", () => {
    const renamedElsewhere: GraphNode = { ...deliverable, title: "Renamed elsewhere" };
    const draft: NodeDraft = { ...draftOf(renamedElsewhere), description: "Ask early." };
    expect(changesOf(renamedElsewhere, { ...draft, title: "Report" }, new Set(["description"])).map((change) => change.field)).toEqual(["description"]);
  });

  it("counts a choice added to a decision as one change, sent as the whole list (A4)", () => {
    const decision: GraphNode = { ...node("decision", "single_choice"), choices: ["one", "two", "three"] };
    const draft: NodeDraft = { ...draftOf(decision), choices: [...draftOf(decision).choices, { id: "report", title: "Industry report" }] };
    expect(changesOf(decision, draft).map((change) => change.field)).toEqual(["choices"]);
  });

  it("replaces a decision whole when its answer type changes, which the engine accepts", async () => {
    const host = await seeded();
    const decision = node("decision", "boolean");
    const draft: NodeDraft = { ...draftOf(decision), answer_type: "single_choice", choices: ["yes", { id: "later", title: "Later" }] };
    const changes = changesOf(decision, draft);
    expect(changes.map((change) => [change.field, change.mutation.op])).toEqual([["answer_type", "replace_node"]]);
    expect(applyToDraft(host.engine, host.deployment(), graphOf(decision), changes.map((change) => change.mutation))).toEqual({ accepted: true });
  });
});
