// B2: answering or revising a decision, with an input for each answer type (A4). The draft
// (the answer being chosen and the revision its author saw) survives a reload. A decision that
// fills a role or pins a milestone is the only way to change that value (E3), and the editor
// says which.
import type { Schema } from "@cairn/client";

import { Button, Field } from "../ui/kit.tsx";
import { NodeLink } from "./parts.tsx";
import { answerable, type AnswerValue, type GraphNode, type NodeDetail, type Ready } from "./model.ts";
import { Rejected } from "./Rejected.tsx";
import { resolveEntity } from "./sections.tsx";
import { useFormDraft, useNodeWrite } from "./write.ts";

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

function Input({ view, node, value, onChange }: { view: Ready; node: GraphNode; value: AnswerValue; onChange: (value: AnswerValue) => void }) {
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

export function AnswerEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `answer:${detail.node.key}`);
  const { node, record, answer } = detail;
  const form = useFormDraft<AnswerValue>(write.journey, node.key, "answer");
  if (!answerable(node.kind, record.state)) {
    return <Rejected view={view} write={write} />;
  }
  const { draft } = form;
  const drives = node.fills_role !== undefined || node.feeds_milestone !== undefined;
  if (draft === undefined) {
    return (
      <span className="row">
        <Button
          primary
          disabled={write.disabled}
          onClick={() => { form.open(startingAnswer(node, resolvedAnswer(view, answer), view.key.today), write.seen); }}
        >
          {answer === undefined ? "Answer" : "Revise the answer"}
        </Button>
        {drives && node.feeds_milestone !== undefined ? (
          <span className="muted">
            Its answer pins <NodeLink view={view} node={node.feeds_milestone} />.
          </span>
        ) : null}
      </span>
    );
  }
  const save = async () => {
    if (await write.run([{ op: "answer", decision: node.key, value: draft.value }], draft)) {
      form.close();
    }
  };
  return (
    <div className="stack" data-testid="answer-editor">
      <span className="row">
        <Input view={view} node={node} value={draft.value} onChange={form.change} />
        <Button primary disabled={write.disabled} onClick={() => void save()}>
          Save the answer
        </Button>
        <Button onClick={() => { form.close(); write.dismiss(); }}>Cancel</Button>
      </span>
      <Rejected view={view} write={write} onResolved={form.close} />
    </div>
  );
}
