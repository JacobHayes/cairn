// B2: answering or revising a decision, the form being the display (design 6.6): the context,
// the question, the input itself with what each choice would do beside it, a reason, and one
// Save. Nothing is preselected; what is picked is a draft that survives a reload and is sent
// only by Save, which is enabled only when the draft differs from what is stored. A decision
// that fills a role or pins a milestone is the only way to change that value (E3), and says so.
// An entity answer picks an entity or names a new one, made in the same patch as the answer
// (B3, E6). A blocked decision shows its form with Save disabled; answering anyway is a guard
// bypass with a reason (D4).
import type { Schema } from "@cairn/client";
import { useId, useMemo, useState, type ReactNode } from "react";

import { useProjected } from "../canvas/hooks.ts";
import { useDraft } from "../data/drafts.ts";
import type { Mutation } from "../data/writes.ts";
import { newEntityKey } from "../people/model.ts";
import { Button, Field } from "../ui/kit.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { effectOf, effectOfOption, outgoing, sameAnswer } from "./answer.ts";
import { answerable, isBlocked, nodeOf, type AnswerValue, type GraphNode, type NodeDetail, type Ready } from "./model.ts";
import { NodeLink, Rationale } from "./parts.tsx";
import { useSavePreview } from "./preview.ts";
import { bypassReady, Rejected } from "./Rejected.tsx";
import { answerText, resolveEntity, roleTitle } from "./sections.tsx";
import { effectWords, savingWords } from "./words.ts";
import { useFormDraft, useNodeWrite, type NodeWrite } from "./write.ts";

type Choice = Schema<"Choice">;
type ChoiceEffect = Schema<"ChoiceEffect">;

const choiceId = (choice: Choice) => (typeof choice === "string" ? choice : choice.id);
const choiceTitle = (choice: Choice) => (typeof choice === "string" ? choice : choice.title);

/** The most choices a single-choice decision lists inline; more are one select. */
const INLINE_SINGLE_MAX = 5;
/** The most choices a multi-choice decision lists inline; more are in a fold. */
const INLINE_MULTI_MAX = 8;
/** The select's value for naming a new person. */
const NEW_PERSON = "__new";

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

/** An answer of the decision's type, as typed (the proposal editor's input, 5.7; the inspector draws its own). */
export function Input({ view, node, value, onChange }: { view: Ready; node: GraphNode; value: AnswerValue; onChange: (value: AnswerValue) => void }) {
  const choices: [string, string][] = (node.choices ?? []).map((choice) => [choiceId(choice), choiceTitle(choice)]);
  const entities: [string, string][] = (view.inputs.deployment.entities ?? []).map((entity) => [entity.key, entity.name]);
  const select = (options: [string, string][], chosen: string, wrap: (id: string) => AnswerValue) => (
    <select aria-label="Answer" value={chosen} onChange={(event) => { onChange(wrap(event.target.value)); }}>
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

/** One choice of the decision: a radio or a checkbox, its title, and what picking it would do. */
function Choose({ type, group, title, checked, effect, onPick }: { type: "radio" | "checkbox"; group?: string; title: string; checked: boolean; effect: ChoiceEffect | undefined; onPick: () => void }) {
  return (
    <label className="choice" data-testid="choice">
      <input type={type} {...(group === undefined ? {} : { name: group })} checked={checked} onChange={onPick} />
      <span>{title}</span>
      {effect === undefined ? null : <span className="choice-effect mono muted">{effectWords(effect)}</span>}
    </label>
  );
}

/** What an input of the decision's type is given: `value` is what is in view (the draft, else the recorded answer). */
interface InputProps {
  view: Ready;
  node: GraphNode;
  value: AnswerValue | undefined;
  pick: (value: AnswerValue) => void;
  effects: readonly ChoiceEffect[] | undefined;
  named: string;
  onNamed: (text: string) => void;
}

const optionsOf = (node: GraphNode): [string, string][] => (node.choices ?? []).map((choice) => [choiceId(choice), choiceTitle(choice)]);

function BooleanInput({ value, pick, effects }: InputProps) {
  // Each form's radios are a group of their own: two decisions on one page never share one.
  const group = useId();
  const options = [
    ["yes", "Yes", true],
    ["no", "No", false],
  ] as const;
  return (
    <div className="choices" role="radiogroup" aria-label="Answer" data-testid="answer">
      {options.map(([id, title, boolean]) => (
        <Choose key={id} type="radio" group={group} title={title} checked={value !== undefined && "boolean" in value && value.boolean === boolean} effect={effectOfOption(effects, id)} onPick={() => { pick({ boolean }); }} />
      ))}
    </div>
  );
}

function SingleInput({ node, value, pick, effects }: InputProps) {
  const group = useId();
  const choices = optionsOf(node);
  const picked = value !== undefined && "single_choice" in value ? value.single_choice : "";
  if (choices.length <= INLINE_SINGLE_MAX) {
    return (
      <div className="choices" role="radiogroup" aria-label="Answer" data-testid="answer">
        {choices.map(([id, title]) => (
          <Choose key={id} type="radio" group={group} title={title} checked={picked === id} effect={effectOfOption(effects, id)} onPick={() => { pick({ single_choice: id }); }} />
        ))}
      </div>
    );
  }
  const effect = effectOfOption(effects, picked);
  return (
    <div className="stack" data-testid="answer">
      <select aria-label="Answer" value={picked} onChange={(event) => { if (event.target.value !== "") { pick({ single_choice: event.target.value }); } }}>
        <option value="">Choose</option>
        {choices.map(([id, title]) => (
          <option key={id} value={id}>
            {title}
          </option>
        ))}
      </select>
      {picked === "" || effect === undefined ? null : <span className="choice-effect mono muted">{effectWords(effect)}</span>}
    </div>
  );
}

function MultiInput({ node, value, pick, effects }: InputProps) {
  const choices = optionsOf(node);
  const picked = value !== undefined && "multi_choice" in value ? value.multi_choice : [];
  const boxes = choices.map(([id, title]) => (
    <Choose key={id} type="checkbox" title={title} checked={picked.includes(id)} effect={effectOfOption(effects, id)} onPick={() => { pick({ multi_choice: toggled(picked, id) }); }} />
  ));
  return choices.length > INLINE_MULTI_MAX ? (
    <details data-testid="answer">
      <summary>Choose ({picked.length} picked)</summary>
      <div className="choices">{boxes}</div>
    </details>
  ) : (
    <div className="choices" role="group" aria-label="Answer" data-testid="answer">
      {boxes}
    </div>
  );
}

function EntityInput({ view, value, pick, named, onNamed }: InputProps) {
  const [creating, setCreating] = useState(false);
  const picked = value !== undefined && "entity" in value ? value.entity : "";
  const naming = creating || named !== "";
  return (
    <div className="stack" data-testid="answer">
      <select
        aria-label="Answer"
        value={naming ? NEW_PERSON : picked}
        onChange={(event) => {
          const next = event.target.value;
          setCreating(next === NEW_PERSON);
          if (next !== NEW_PERSON) {
            onNamed("");
          }
          pick({ entity: next === NEW_PERSON ? "" : next });
        }}
      >
        <option value="">Choose</option>
        {(view.inputs.deployment.entities ?? []).map((entity) => (
          <option key={entity.key} value={entity.key}>
            {entity.name}
          </option>
        ))}
        <option value={NEW_PERSON}>New person</option>
      </select>
      {naming ? <Field aria-label="New person's name" placeholder="Their name" value={named} onChange={(event) => { onNamed(event.target.value); }} /> : null}
    </div>
  );
}

function EntityListInput({ view, value, pick, named, onNamed }: InputProps) {
  const picked = value !== undefined && "entity_list" in value ? value.entity_list : [];
  return (
    <div className="stack" data-testid="answer">
      <div className="choices" role="group" aria-label="Answer">
        {(view.inputs.deployment.entities ?? []).map((entity) => (
          <Choose key={entity.key} type="checkbox" title={entity.name} checked={picked.includes(entity.key)} effect={undefined} onPick={() => { pick({ entity_list: toggled(picked, entity.key) }); }} />
        ))}
      </div>
      <Field
        aria-label="New person's name"
        placeholder="Add a new person"
        value={named}
        onChange={(event) => {
          onNamed(event.target.value);
          if (value === undefined) {
            pick({ entity_list: [] });
          }
        }}
      />
    </div>
  );
}

/** The input for the decision's type, each choice with what it would do beside it. */
function AnswerInput(props: InputProps) {
  const { node, value, pick } = props;
  switch (node.answer_type) {
    case "boolean":
      return <BooleanInput {...props} />;
    case "single_choice":
      return <SingleInput {...props} />;
    case "multi_choice":
      return <MultiInput {...props} />;
    case "date":
      return <Field type="date" aria-label="Answer" data-testid="answer" value={value !== undefined && "date" in value ? value.date : ""} onChange={(event) => { pick({ date: event.target.value }); }} />;
    case "entity":
      return <EntityInput {...props} />;
    case "entity_list":
      return <EntityListInput {...props} />;
    default:
      return (
        <textarea className="grow" aria-label="Answer" data-testid="answer" rows={1} value={value !== undefined && "text" in value ? value.text : ""} onChange={(event) => { pick({ text: event.target.value }); }} />
      );
  }
}

/** B2: the reason, open and optional under a new answer; on a recorded one, shown, with a way to edit it. */
function Why({ typed, recorded, editing, onChange, onEdit }: {
  typed: string | undefined;
  recorded: string | undefined;
  editing: boolean;
  onChange: (text: string) => void;
  onEdit: (() => void) | undefined;
}) {
  if (!editing) {
    return (
      <div className="stack" data-testid="answer-reason">
        <Rationale text={recorded} />
        {onEdit === undefined ? null : (
          <span>
            <Button onClick={onEdit}>{recorded === undefined ? "Add a reason" : "Edit reason"}</Button>
          </span>
        )}
      </div>
    );
  }
  const text = typed ?? "";
  return (
    <div className="stack" data-testid="answer-why">
      <label className="field">
        <span className="small muted">Why (optional)</span>
        <textarea className="grow" aria-label="Why" rows={1} value={text} onChange={(event) => { onChange(event.target.value); }} />
      </label>
      {recorded === undefined || recorded === text ? null : (
        <span className="muted small row">
          Previous reason: &ldquo;{recorded.length > 80 ? `${recorded.slice(0, 80)}...` : recorded}&rdquo;
          <Button onClick={() => { onChange(recorded); }}>Reuse</Button>
        </span>
      )}
    </div>
  );
}

/** D4: the reason a blocked decision is answered past its guard with, as a draft. */
export type Anyway = ReturnType<typeof useFormDraft<string>>;

/** B2: whether someone else answered while this draft was made (what was recorded when it began is no longer), and the way to keep the draft anyway. */
function useAnsweredMeanwhile(write: NodeWrite, node: string, form: ReturnType<typeof useFormDraft<AnswerValue>>, stored: AnswerValue | undefined) {
  const began = useFormDraft<AnswerValue | null>(write.journey, node, "answer-from");
  return {
    answered: form.draft !== undefined && began.draft !== undefined && !sameAnswer(form.draft.value, stored) && !sameAnswer(began.draft.value ?? undefined, stored),
    begin: () => { began.open(stored ?? null, write.seen); },
    keep: () => {
      if (form.draft !== undefined) {
        form.open(form.draft.value, write.seen);
      }
      began.open(stored ?? null, write.seen);
    },
    close: began.close,
  };
}

/** B2: the callout that replaces Save when someone else answered first. */
function AnsweredMeanwhile({ text, onKeep, onDiscard }: { text: string; onKeep: () => void; onDiscard: () => void }) {
  return (
    <div className="stack callout" data-testid="answered-meanwhile">
      <span>Someone else answered {text} while you were choosing.</span>
      <span className="row">
        <Button primary onClick={onKeep}>Use mine</Button>
        <Button onClick={onDiscard}>Discard mine</Button>
      </span>
    </div>
  );
}

/** What the form holds, and what it would do: the drafts, what Save would send, and Save itself. */
function useAnswerForm(view: Ready, detail: NodeDetail, anyway: Anyway | undefined) {
  const write = useNodeWrite(view, `answer:${detail.node.key}`, detail.node.key);
  // The new entity's name is part of the answer's draft: kept across a reload, gone with it.
  const [namedDraft, setNamed] = useDraft<string>(`answer-entity:${write.journey}:${detail.node.key}`);
  const { node, record, answer, rationale } = detail;
  const form = useFormDraft<AnswerValue>(write.journey, node.key, "answer");
  const why = useFormDraft<string>(write.journey, node.key, "answer-why");
  const open = answerable(node.kind, record.state);
  const projected = useProjected(view, open ? { projection: "answer_effects", key: node.key } : undefined);
  const effects = projected.value?.applies === true ? projected.value.choices : undefined;
  const stored = resolvedAnswer(view, answer);
  const meanwhile = useAnsweredMeanwhile(write, node.key, form, stored);
  const named = namedDraft ?? "";
  // Minted once per name, so a preview of the draft is not invalidated by every render.
  const entityKey = useMemo(() => (named.trim() === "" ? "" : newEntityKey(named)), [named]);
  const sending = outgoing({ stored, storedReason: rationale, draft: form.draft?.value, reason: why.draft?.value, named });
  const mutations = sending === undefined ? undefined : answerMutations(node.key, sending.value, named, sending.reason, () => entityKey);
  const caused = useSavePreview(view, mutations);
  const bypass = anyway?.draft?.value;
  const bypassing = bypass !== undefined;
  const newAnswer = form.draft !== undefined && !sameAnswer(form.draft.value, stored);
  const blocked = isBlocked(detail.derived, view);
  const canSave = sending !== undefined && !write.disabled && (!blocked || (bypassing && bypassReady(bypass)));
  const close = () => {
    setNamed(undefined);
    form.close();
    why.close();
    meanwhile.close();
    anyway?.close();
  };
  return {
    write,
    form,
    why,
    named,
    setNamed,
    stored,
    effects,
    newAnswer,
    blocked,
    bypass,
    canSave,
    close,
    open,
    meanwhile,
    preview: form.draft === undefined || !newAnswer ? undefined : savingWords(effectOf(effects, form.draft.value), caused, (key) => nodeOf(view, key)?.title ?? key),
    pick: (value: AnswerValue) => {
      if (form.draft === undefined) {
        form.open(value, write.seen);
        meanwhile.begin();
      } else {
        form.change(value);
      }
      // A different answer is a new answer: the reason inherited from "Edit reason" goes (B2).
      if (!sameAnswer(value, stored) && why.draft !== undefined && why.draft.value === rationale) {
        why.open("", form.draft ?? write.seen);
      }
    },
    save: async () => {
      if (!canSave || mutations === undefined) {
        return;
      }
      const override: Mutation[] = bypass === undefined ? [] : [{ op: "apply_override", node: node.key, override: { guard_bypass: { guards: ["deps_done"], reason: bypass.trim() } } }];
      if (await write.run([...override, ...mutations], form.draft ?? why.draft ?? write.seen)) {
        close();
      }
    },
  };
}

/**
 * The decision's answer form. `menu` ends its action row (the inspector's `⋯`); `anyway` is the
 * bypass's reason draft, opened from that menu on a blocked decision.
 */
export function AnswerEditor({ view, detail, menu, anyway }: { view: Ready; detail: NodeDetail; menu?: ReactNode; anyway?: Anyway }) {
  const state = useAnswerForm(view, detail, anyway);
  const { node, rationale } = detail;
  const { write, form, why, stored, bypass } = state;
  if (!state.open) {
    return <Rejected view={view} write={write} />;
  }
  const bypassing = bypass !== undefined;
  const asked = node.prompt !== undefined;
  const description = node.description ?? "";
  return (
    <div
      className="stack answer-form"
      data-testid="answer-editor"
      onKeyDown={(event) => {
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          void state.save();
        }
      }}
    >
      {asked && description !== "" ? <Markdown text={description} data-testid="description" /> : null}
      <Markdown className="question" text={asked ? (node.prompt ?? "") : description} data-testid="prompt" />
      {node.help === undefined ? null : <Markdown className="muted small" text={node.help} />}
      <AnswerInput view={view} node={node} value={form.draft?.value ?? stored} pick={state.pick} effects={state.effects} named={state.named} onNamed={(text) => { state.setNamed(text === "" ? undefined : text); }} />
      {node.fills_role === undefined ? null : <span className="muted small">Your answer fills the role {roleTitle(view, node.fills_role)}.</span>}
      {node.feeds_milestone === undefined ? null : (
        <span className="muted small">
          Your answer pins <NodeLink view={view} node={node.feeds_milestone} />.
        </span>
      )}
      <Why
        typed={why.draft?.value}
        recorded={rationale}
        editing={state.newAnswer || why.draft !== undefined}
        onChange={(text) => { why.open(text, form.draft ?? write.seen); }}
        onEdit={stored === undefined ? undefined : () => { why.open(rationale ?? "", write.seen); }}
      />
      {state.preview?.info === undefined ? null : <span data-testid="preview">{state.preview.info}</span>}
      {state.preview?.warning === undefined ? null : <span className="muted" data-testid="preview-warning">{state.preview.warning}</span>}
      {bypassing && anyway !== undefined ? (
        <span className="row" data-testid="answer-anyway">
          <Field aria-label="Why answer anyway" placeholder="Why (required)" value={bypass} onChange={(event) => { anyway.change(event.target.value); }} />
          <Button onClick={() => { anyway.close(); }}>Cancel</Button>
        </span>
      ) : null}
      {state.meanwhile.answered ? (
        <AnsweredMeanwhile text={stored === undefined ? "this" : answerText(view, stored, node)} onKeep={state.meanwhile.keep} onDiscard={state.close} />
      ) : (
        <span className="row actions-row">
          <Button primary disabled={!state.canSave} onClick={() => void state.save()}>
            {bypassing ? "Save anyway" : stored === undefined ? "Save" : "Save change"}
          </Button>
          <kbd className="muted">⌘↵</kbd>
          <span className="spacer" />
          {menu}
        </span>
      )}
      <Rejected view={view} write={write} onResolved={state.close} />
    </div>
  );
}
