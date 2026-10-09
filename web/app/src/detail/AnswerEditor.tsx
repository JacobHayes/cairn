// B2: answering or revising a decision, with an input for each answer type (A4). The draft
// (the answer being chosen and the revision its author saw) survives a reload. A decision that
// fills a role or pins a milestone is the only way to change that value (E3), and the editor
// says which. An entity answer picks existing entities or names a new one, made in the same
// patch as the answer (B3, E6: "answer this decision with a new person" is one patch).
// B2: an answer may carry a rationale, a markdown "why" that belongs to that answer alone. The
// field is open and starts empty (nothing is carried forward silently); the previous reason is
// offered, and reused only on a click. Editing just the reason of the current answer is a
// revision that starts from that reason.
import type { Schema } from "@cairn/client";
import { useDraft } from "../data/drafts.ts";

import type { Mutation } from "../data/writes.ts";
import { newEntityKey } from "../people/model.ts";

import { Button, Field } from "../ui/kit.tsx";
import { NodeLink } from "./parts.tsx";
import { answerable, type AnswerValue, type GraphNode, type NodeDetail, type Ready } from "./model.ts";
import { Rejected } from "./Rejected.tsx";
import { resolveEntity } from "./sections.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "./write.ts";

type Choice = Schema<"Choice">;

const choiceId = (choice: Choice) => (typeof choice === "string" ? choice : choice.id);
const choiceTitle = (choice: Choice) => (typeof choice === "string" ? choice : choice.title);

/** The answer an editor starts from: the current one, or an empty one of the node's type. */
export function startingAnswer(node: GraphNode, current: AnswerValue | undefined, today: string): AnswerValue {
  if (current !== undefined) {
    return current;
  }
  const first = (node.choices ?? [])[0];
  switch (node.answer_type) {
    case "boolean":
      return { boolean: true };
    case "single_choice":
      return { single_choice: first === undefined ? "" : choiceId(first) };
    case "multi_choice":
      return { multi_choice: [] };
    case "date":
      return { date: today };
    case "entity":
      return { entity: "" };
    case "entity_list":
      return { entity_list: [] };
    default:
      return { text: "" };
  }
}

/** E6: an entity answer as the deployment names its entities now, through any merge. */
export function resolvedAnswer(view: Ready, answer: AnswerValue | undefined): AnswerValue | undefined {
  if (answer !== undefined && "entity" in answer) {
    return { entity: resolveEntity(view, answer.entity) };
  }
  if (answer !== undefined && "entity_list" in answer) {
    return { entity_list: [...new Set(answer.entity_list.map((key) => resolveEntity(view, key)))] };
  }
  return answer;
}

function toggled(list: string[], item: string): string[] {
  return list.includes(item) ? list.filter((each) => each !== item) : [...list, item];
}

function Checks({ options, chosen, onChange }: { options: [string, string][]; chosen: string[]; onChange: (next: string[]) => void }) {
  return (
    <span className="row">
      {options.map(([id, title]) => (
        <label key={id}>
          <input type="checkbox" checked={chosen.includes(id)} onChange={() => { onChange(toggled(chosen, id)); }} /> {title}
        </label>
      ))}
    </span>
  );
}

/** An answer of the decision's type, as typed (also the proposal editor's, 5.7). */
export function Input({ view, node, value, onChange }: { view: Ready; node: GraphNode; value: AnswerValue; onChange: (value: AnswerValue) => void }) {
  const choices: [string, string][] = (node.choices ?? []).map((choice) => [choiceId(choice), choiceTitle(choice)]);
  const entities: [string, string][] = (view.inputs.deployment.entities ?? []).map((entity) => [entity.key, entity.name]);
  const select = (options: [string, string][], chosen: string, wrap: (id: string) => AnswerValue) => (
    <select className="select" aria-label="Answer" value={chosen} onChange={(event) => { onChange(wrap(event.target.value)); }}>
      <option value="">Choose</option>
      {options.map(([id, title]) => (
        <option key={id} value={id}>
          {title}
        </option>
      ))}
    </select>
  );
  if ("boolean" in value) {
    return select([["yes", "Yes"], ["no", "No"]], value.boolean ? "yes" : "no", (id) => ({ boolean: id === "yes" }));
  }
  if ("single_choice" in value) {
    return select(choices, value.single_choice, (id) => ({ single_choice: id }));
  }
  if ("multi_choice" in value) {
    return <Checks options={choices} chosen={value.multi_choice} onChange={(next) => { onChange({ multi_choice: next }); }} />;
  }
  if ("entity" in value) {
    return select(entities, value.entity, (id) => ({ entity: id }));
  }
  if ("entity_list" in value) {
    return <Checks options={entities} chosen={value.entity_list} onChange={(next) => { onChange({ entity_list: next }); }} />;
  }
  if ("date" in value) {
    return <Field type="date" aria-label="Answer" value={value.date} onChange={(event) => { onChange({ date: event.target.value }); }} />;
  }
  return <Field aria-label="Answer" value={value.text} onChange={(event) => { onChange({ text: event.target.value }); }} />;
}

/**
 * B3, B2: the answer's mutations: the answer alone, or, when a new entity is named for an entity
 * answer, the entity made first and the answer naming it (added to a list answer). A blank
 * `rationale` is none: the field is left off, as the server refuses blank text.
 */
export function answerMutations(decision: string, value: AnswerValue, named: string, rationale: string, key: (name: string) => string = newEntityKey): Mutation[] {
  const why = rationale.trim();
  const reason = why === "" ? {} : { rationale: why };
  const name = named.trim();
  if (name === "" || !("entity" in value || "entity_list" in value)) {
    return [{ op: "answer", decision, value, ...reason }];
  }
  const entity = key(name);
  const answer: AnswerValue = "entity" in value ? { entity } : { entity_list: [...value.entity_list, entity] };
  return [{ op: "create_entity", entity: { key: entity, name } }, { op: "answer", decision, value: answer, ...reason }];
}

/** B2: the "why" field, open under the input, with the previous reason offered for reuse. */
function WhyField({ value, previous, onChange }: { value: string; previous: string | undefined; onChange: (text: string) => void }) {
  return (
    <div className="stack" data-testid="answer-why">
      <textarea className="textarea" aria-label="Why" placeholder="Why (optional, markdown)" value={value} onChange={(event) => { onChange(event.target.value); }} />
      {previous === undefined || previous === value ? null : (
        <span className="muted row">
          Previous reason: &ldquo;{previous.length > 80 ? `${previous.slice(0, 80)}...` : previous}&rdquo;
          <Button onClick={() => { onChange(previous); }}>Reuse</Button>
        </span>
      )}
    </div>
  );
}

type AnswerForm = ReturnType<typeof useFormDraft<AnswerValue>>;
type WhyForm = ReturnType<typeof useFormDraft<string>>;

/** The open editor: the answer's input, the why field, and save. */
function Editing({ view, detail, write, form, why, named, onNamed, close }: {
  view: Ready;
  detail: NodeDetail;
  write: NodeWrite;
  form: AnswerForm & { draft: NonNullable<AnswerForm["draft"]> };
  why: WhyForm;
  named: string;
  onNamed: (text: string | undefined) => void;
  close: () => void;
}) {
  const { node, answer, rationale } = detail;
  const { draft } = form;
  const reason = why.draft?.value ?? "";
  // Opened (again) against the draft's revisions, so a draft saved before reasons existed has one.
  const setWhy = (text: string) => { why.open(text, draft); };
  // A different answer is a new answer: the reason it inherited from "Edit reason" goes (B2).
  const startNew = () => {
    if (rationale !== undefined && reason === rationale) {
      setWhy("");
    }
  };
  const change = (value: AnswerValue) => {
    form.change(value);
    if (JSON.stringify(value) !== JSON.stringify(resolvedAnswer(view, answer))) {
      startNew();
    }
  };
  const save = async () => {
    if (await write.run(answerMutations(node.key, draft.value, named, reason), draft)) {
      close();
    }
  };
  const entityAnswer = "entity" in draft.value || "entity_list" in draft.value;
  return (
    <div className="stack" data-testid="answer-editor">
      <span className="row">
        <Input view={view} node={node} value={draft.value} onChange={change} />
        {entityAnswer ? (
          <Field aria-label="Or a new entity" placeholder={"entity" in draft.value ? "Or a new entity's name" : "And a new entity's name"} value={named} onChange={(event) => {
            onNamed(event.target.value === "" ? undefined : event.target.value);
            startNew();
          }} />
        ) : null}
        <Button primary disabled={write.disabled} onClick={() => void save()}>
          Save the answer
        </Button>
        <Button onClick={() => { close(); write.dismiss(); }}>Cancel</Button>
      </span>
      <WhyField value={reason} previous={rationale} onChange={setWhy} />
      <Rejected view={view} write={write} onResolved={close} />
    </div>
  );
}

export function AnswerEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `answer:${detail.node.key}`);
  // The new entity's name is part of the answer's draft: kept across a reload, gone with it.
  const [namedDraft, setNamed] = useDraft<string>(`answer-entity:${write.journey}:${detail.node.key}`);
  const { node, record, answer, rationale } = detail;
  const form = useFormDraft<AnswerValue>(write.journey, node.key, "answer");
  const why = useFormDraft<string>(write.journey, node.key, "answer-why");
  if (!answerable(node.kind, record.state)) {
    return <Rejected view={view} write={write} />;
  }
  const close = () => {
    setNamed(undefined);
    form.close();
    why.close();
  };
  const begin = (reason: string) => {
    form.open(startingAnswer(node, resolvedAnswer(view, answer), view.key.today), write.seen);
    why.open(reason, write.seen);
  };
  if (form.draft !== undefined) {
    return <Editing view={view} detail={detail} write={write} form={{ ...form, draft: form.draft }} why={why} named={namedDraft ?? ""} onNamed={setNamed} close={close} />;
  }
  return (
    <span className="row">
      {/* A new answer starts with no reason of its own (B2). */}
      <Button primary disabled={write.disabled} onClick={() => { begin(""); }}>
        {answer === undefined ? "Answer" : "Revise the answer"}
      </Button>
      {answer !== undefined && rationale !== undefined ? (
        <Button disabled={write.disabled} onClick={() => { begin(rationale); }}>
          Edit reason
        </Button>
      ) : null}
      {node.feeds_milestone === undefined ? null : (
        <span className="muted">
          Its answer pins <NodeLink view={view} node={node.feeds_milestone} />.
        </span>
      )}
    </span>
  );
}
