// The fields of a node form, grouped as a kind has them (A1a): the shared ones, work's
// estimate and flags, a decision's prompt, answer type, choices, and what it fills or pins,
// a milestone's flags, and a stage's bounds. Each field shows what stops it being sent and
// what the engine said about it, at the field (A15).
import { useState, type ReactNode } from "react";

import { Button, Field } from "../ui/kit.tsx";
import { choiceId, choiceLabel, fieldsOf, type Choice, type FormField, type NodeDraft } from "./fields.ts";
import { nodesByPath, type AnswerType, type NodeKind, type Role, type Tree } from "./graph.ts";
import { slugOf, uniqueId } from "./keys.ts";
import { FieldBox, Picker, type FieldNotes } from "./parts.tsx";
import { StageBoundsEditor } from "./DateRuleEditor.tsx";

/** What every field group is handed. */
export interface FieldsProps {
  kind: NodeKind;
  nodeKey: string;
  draft: NodeDraft;
  change: (next: Partial<NodeDraft>) => void;
  notes: (field: FormField) => FieldNotes;
  tree: Tree;
  roles: readonly Role[];
}

const ANSWER_TYPES: { value: AnswerType; label: string }[] = [
  { value: "boolean", label: "yes or no" },
  { value: "single_choice", label: "one choice" },
  { value: "multi_choice", label: "several choices" },
  { value: "text", label: "text" },
  { value: "date", label: "a date" },
  { value: "entity", label: "a person or team" },
  { value: "entity_list", label: "people or teams" },
];

/** Whether the kind (and answer type) has `field`, so its control is offered (A1a). */
export function has(props: Pick<FieldsProps, "kind" | "draft">, field: FormField): boolean {
  return fieldsOf(props.kind, props.draft.answer_type).includes(field);
}

function Text({ props, field, label, area = false }: { props: FieldsProps; field: "id" | "title" | "description" | "prompt" | "help"; label: string; area?: boolean }) {
  const value = props.draft[field];
  const onChange = (next: string) => {
    props.change({ [field]: next });
  };
  return (
    <FieldBox label={label} place={field} notes={props.notes(field)}>
      {area ? (
        <textarea aria-label={label} value={value} onChange={(event) => { onChange(event.target.value); }} />
      ) : (
        <Field aria-label={label} value={value} onChange={(event) => { onChange(event.target.value); }} />
      )}
    </FieldBox>
  );
}

function Days({ props, field, label, hint }: { props: FieldsProps; field: "weight" | "estimate"; label: string; hint: string }) {
  return (
    <FieldBox label={label} place={field} notes={props.notes(field)}>
      <Field type="number" min={0} aria-label={label} placeholder={hint} value={props.draft[field]} onChange={(event) => { props.change({ [field]: event.target.value }); }} />
    </FieldBox>
  );
}

function Flag({ props, field, label }: { props: FieldsProps; field: "requires_artifact" | "requires_note" | "placeholder" | "final" | "auto_reach"; label: string }) {
  return (
    <FieldBox label="" place={field} notes={props.notes(field)}>
      <label className="row">
        <input type="checkbox" aria-label={label} checked={props.draft[field]} onChange={(event) => { props.change({ [field]: event.target.checked }); }} />
        {label}
      </label>
    </FieldBox>
  );
}

/** The fields every kind has (A1a): id, title, description, weight. */
export function SharedFields({ props }: { props: FieldsProps }) {
  return (
    <>
      <Text props={props} field="title" label="Title" />
      <Text props={props} field="id" label="Id" />
      <Text props={props} field="description" label="Description" area />
      <Days props={props} field="weight" label="Weight" hint={props.kind === "group" ? "0 (a group's default)" : "1 (the default)"} />
    </>
  );
}

/** Work's own fields: an estimate, an artifact or a note required before done, and placeholder (A16). */
export function WorkFields({ props }: { props: FieldsProps }) {
  return (
    <>
      {has(props, "estimate") ? <Days props={props} field="estimate" label="Estimate (days)" hint="none" /> : null}
      {has(props, "requires_artifact") ? <Flag props={props} field="requires_artifact" label="Requires an artifact before done" /> : null}
      {has(props, "requires_note") ? <Flag props={props} field="requires_note" label="Requires a note before done" /> : null}
      {has(props, "placeholder") ? <Flag props={props} field="placeholder" label="Placeholder: each journey breaks it down" /> : null}
      {has(props, "final") ? <Flag props={props} field="final" label="The final milestone" /> : null}
      {has(props, "auto_reach") ? <Flag props={props} field="auto_reach" label="Reached by itself on its date" /> : null}
    </>
  );
}

/** A4: a choice decision's choices, each an id and a label. */
export function ChoicesEditor({ props }: { props: FieldsProps }) {
  const choices = props.draft.choices;
  const set = (next: Choice[]) => {
    props.change({ choices: next });
  };
  const label = (choice: Choice, title: string): Choice => (title.trim() === "" || title === choiceId(choice) ? choiceId(choice) : { id: choiceId(choice), title });
  return (
    <FieldBox label="Choices" place="choices" notes={props.notes("choices")}>
      {choices.map((choice, at) => (
        <span key={at} className="row" data-testid="choice">
          <Field aria-label="Choice label" value={choiceLabel(choice)} onChange={(event) => { set(choices.map((each, index) => (index === at ? label(each, event.target.value) : each))); }} />
          <span className="muted mono">{choiceId(choice)}</span>
          <Button aria-label={`Remove the choice ${choiceLabel(choice)}`} onClick={() => { set(choices.filter((_, index) => index !== at)); }}>
            Remove
          </Button>
        </span>
      ))}
      <AddChoice choices={choices} onAdd={(choice) => { set([...choices, choice]); }} />
    </FieldBox>
  );
}

/** A new choice, added to the draft only: no form of its own, so it never submits the node's form. */
function AddChoice({ choices, onAdd }: { choices: readonly Choice[]; onAdd: (choice: Choice) => void }) {
  const [title, setTitle] = useState("");
  const add = () => {
    const trimmed = title.trim();
    if (trimmed !== "") {
      const id = uniqueId(slugOf(trimmed, "choice"), new Set(choices.map(choiceId)));
      onAdd(id === trimmed ? id : { id, title: trimmed });
      setTitle("");
    }
  };
  return (
    <span className="row">
      <Field
        aria-label="New choice"
        placeholder="A new choice"
        value={title}
        onChange={(event) => { setTitle(event.target.value); }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            add();
          }
        }}
      />
      <Button disabled={title.trim() === ""} onClick={add}>Add the choice</Button>
    </span>
  );
}

/** A4, A6, E3: a decision's prompt, help, answer type, choices, and the role it fills or the milestone it pins. */
export function DecisionFields({ props }: { props: FieldsProps }) {
  const multi = props.draft.answer_type === "entity_list";
  const roles = props.roles.filter((role) => (role.multi ?? false) === multi);
  const milestones = nodesByPath(props.tree, "milestone");
  return (
    <>
      <Text props={props} field="prompt" label="Prompt" area />
      <Text props={props} field="help" label="Help" area />
      <FieldBox label="Answer type" place="answer_type" notes={props.notes("answer_type")}>
        <Picker aria-label="Answer type" value={props.draft.answer_type} options={ANSWER_TYPES} onChange={(event) => { props.change({ answer_type: event.target.value as AnswerType }); }} />
      </FieldBox>
      {has(props, "choices") ? <ChoicesEditor props={props} /> : null}
      {has(props, "fills_role") ? (
        <FieldBox label="Fills the role" place="fills_role" notes={props.notes("fills_role")}>
          <Picker aria-label="Fills the role" value={props.draft.fills_role} none="No role" options={roles.map((role) => ({ value: role.key, label: role.title ?? role.id }))} onChange={(event) => { props.change({ fills_role: event.target.value }); }} />
        </FieldBox>
      ) : null}
      {has(props, "feeds_milestone") ? (
        <FieldBox label="Pins the milestone" place="feeds_milestone" notes={props.notes("feeds_milestone")}>
          <Picker aria-label="Pins the milestone" value={props.draft.feeds_milestone} none="No milestone" options={milestones.map((milestone) => ({ value: milestone.key, label: milestone.title }))} onChange={(event) => { props.change({ feeds_milestone: event.target.value }); }} />
        </FieldBox>
      ) : null}
    </>
  );
}

/** F4: a stage's bounds, on a group. */
export function StageFields({ props }: { props: FieldsProps }) {
  return (
    <StageBoundsEditor
      opensAt={props.draft.opens_at}
      closesAt={props.draft.closes_at}
      gates={props.draft.gates}
      closes={props.draft.closes}
      tree={props.tree}
      group={props.nodeKey}
      notes={{ opens_at: props.notes("opens_at"), closes_at: props.notes("closes_at") }}
      onChange={(next) => { props.change(next); }}
    />
  );
}

/** A labeled group of fields in the form. */
export function FieldGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <fieldset className="stack author-group">
      <legend className="author-label">{title}</legend>
      {children}
    </fieldset>
  );
}
