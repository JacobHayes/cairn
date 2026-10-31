// C14: a proposal's changes, editable one by one: each mutation in words with its own editor
// where it has one (a node it adds with the authoring field editors, a field it sets with the
// condition or date-rule editor, a pin's date, an answer with the answer input) and a way to
// drop it; and what a reviewer adds: a node (B10: a breakdown's pieces), a node removed with
// its cascade (A18), and an edge (A3). Nothing is sent from here: the edits change the draft,
// previewed at once, and are saved with the proposal.
import { useEffect, useRef, useState } from "react";

import { ConditionEditor } from "../authoring/ConditionEditor.tsx";
import { removalMutations, useRemovalPlan } from "../authoring/removal.ts";
import { DateRuleEditor } from "../authoring/DateRuleEditor.tsx";
import { draftOf, nodeFrom, valueOf, type FormField, type NodeDraft } from "../authoring/fields.ts";
import { useDraftProblems } from "../authoring/form-problems.ts";
import { NewNodeFields, useNewNodeCheck, type NewNodeValues } from "../authoring/NewNodeFields.tsx";
import { moveTargets, newNode, nodesByPath, nodesFor, pathOf, rolesForFill, type GraphNode, type Mutation, type NodeField, type ReferenceField, type Role, type Tree } from "../authoring/graph.ts";
import { BasicsFields, ChecklistEditor, type NewItem, ChoicesEditor, CompletionFields, DecisionFields, WeightFields } from "../authoring/NodeFields.tsx";
import { CheckFailed, FieldBox, Picker } from "../authoring/parts.tsx";
import { useDraft } from "../data/drafts.ts";
import type { Deployment } from "../data/host.ts";
import { subjectOf } from "../data/writes.ts";
import { Input as AnswerInput } from "../detail/AnswerEditor.tsx";
import { checklistOf, type Ready } from "../detail/model.ts";
import { Section } from "../detail/parts.tsx";
import { fieldWords } from "../status/words.ts";
import { NODE_FIELDS } from "@cairn/client";
import { Button, Input, PickBox } from "../ui/kit.tsx";
import { WhenInput } from "../ui/WhenInput.tsx";
import { carryIds, withFieldValue, withMutation, withMutationsAdded, withoutMutation, type ProposalDraft } from "./model.ts";
import { addNodeKey, isNewNodeValues, useNodeTyped } from "./typed.ts";
import { mutationWords, type Names } from "./words.ts";

/** What the change editors read and change. */
export interface ChangesContext {
  draft: ProposalDraft;
  edit: (next: ProposalDraft) => void;
  names: Names;
  /** The destination's graph as the proposal leaves it (as it is, when there is no preview). */
  tree: Tree;
  roles: readonly Role[];
  deployment: Deployment;
  today: string;
  /** The journey the proposal is for, derived, when it is one: answers read it. */
  ready: Ready | undefined;
  editable: boolean;
  /** The destination has a graph to add nodes and edges to (the deployment's identities have none). */
  hasGraph: boolean;
  /** The proposal being reviewed: what a reviewer has typed and not yet added is kept under it. */
  proposal: string;
  /** Where a new node goes unless another is picked (a breakdown's placeholder). */
  defaultParent: string | undefined;
  /** The editors holding a value with a problem, or one the engine has not passed yet. */
  invalid: ReadonlySet<string>;
  /** Tells the review an editor holds a value with a problem (or no longer does), so nothing is saved or applied meanwhile. */
  setInvalid: (editor: string, invalid: boolean) => void;
}

/** Reports `invalid` for `editor` while it is shown, and clears it when the editor goes. */
function useInvalid(context: ChangesContext, editor: string, invalid: boolean): void {
  const { setInvalid } = context;
  useEffect(() => {
    setInvalid(editor, invalid);
  }, [setInvalid, editor, invalid]);
  useEffect(() => () => { setInvalid(editor, false); }, [setInvalid, editor]);
}

/**
 * An editor's draft of `source`, kept as typed (a value with a problem stays here, unwritten)
 * and made afresh from `source` whenever it is replaced by anything but this editor's own
 * write: the reviewer's edits dropped, or the proposal saved or refreshed by someone else.
 * `wrote` names what the editor is about to write, so its own write keeps the draft.
 */
function useSyncedDraft<T>(source: unknown, make: () => T): { draft: T; set: (draft: T) => void; wrote: (source: unknown) => void } {
  const key = JSON.stringify(source);
  const [held, setHeld] = useState(() => ({ key, draft: make() }));
  const written = useRef<string | undefined>(undefined);
  let current = held;
  if (held.key !== key) {
    current = { key, draft: written.current === key ? held.draft : make() };
    setHeld(current);
  }
  return {
    draft: current.draft,
    set: (draft) => { setHeld({ key: current.key, draft }); },
    wrote: (next) => { written.current = JSON.stringify(next); },
  };
}

/** The checklist's new-item text, held by the editor while it is open. */
function useKeptItem(): [string, (value: string) => void] {
  return useState("");
}

/** The editor's new-item field over the held text: adding makes the change and clears the text. */
function newItemOf([value, set]: [string, (value: string) => void], change: (next: Partial<NodeDraft>) => void): NewItem {
  return { value, set, add: (next) => { change(next); set(""); } };
}

/** A node the proposal adds, edited with authoring's fields; the mutation follows once nothing stops it. */
function AddedNode({ editor, node, onChange, context }: { editor: string; node: GraphNode; onChange: (node: GraphNode) => void; context: ChangesContext }) {
  const synced = useSyncedDraft<NodeDraft>(node, () => draftOf(node));
  const kept = useKeptItem();
  const draft = synced.draft;
  const form = useDraftProblems(node.kind, draft);
  const { checked, problems } = form;
  useInvalid(context, editor, Object.keys(problems).length > 0 || !checked);
  const edited = useRef<NodeDraft | undefined>(undefined);
  const change = (next: Partial<NodeDraft>) => {
    const merged = { ...draft, ...next };
    edited.current = merged;
    synced.set(merged);
  };
  // An edit is written once the engine has read it and found nothing wrong: the one check shows the messages and gates the write.
  useEffect(() => {
    if (edited.current === draft && checked && Object.keys(problems).length === 0) {
      edited.current = undefined;
      const written = nodeFrom(node, draft);
      synced.wrote(written);
      onChange(written);
    }
  });
  const props = {
    kind: node.kind,
    nodeKey: node.key,
    draft,
    change,
    notes: (field: FormField) => ({ problem: problems[field] }),
    tree: context.tree,
    roles: context.roles,
    newItem: newItemOf(kept, change),
  };
  return (
    <div className="stack" data-testid="added-node-form">
      <CheckFailed failure={form.failure} retry={form.retry} />
      <Section title="Basics" open testId="added-basics">
        <BasicsFields props={props} />
      </Section>
      {node.kind === "decision" ? (
        <Section title="Question" open testId="added-question">
          <DecisionFields props={props} />
        </Section>
      ) : null}
      <Section title="Relevance and dates" open={problems.due_by !== undefined || problems.not_before !== undefined} testId="added-dates">
        <ConditionEditor value={draft.relevant_when} onChange={(next) => { change({ relevant_when: next }); }} tree={context.tree} node={node.key} identities={context.deployment.identities ?? []} today={context.today} />
        {(["due_by", "not_before"] as const).map((which) => (
          <FieldBox key={which} label={fieldWords(which)} place={which} notes={{ problem: problems[which] }}>
            <DateRuleEditor which={which} value={draft[which]} onChange={(next) => { change({ [which]: next }); }} tree={context.tree} node={node.key} />
          </FieldBox>
        ))}
      </Section>
      <Section title="Completion needs, weight and estimate" testId="added-weight">
        <CompletionFields props={props} />
        <WeightFields props={props} />
      </Section>
    </div>
  );
}

/** The node a set field is on, as the proposal leaves it: one it adds, or the graph's. */
function nodeOf(context: ChangesContext, key: string): GraphNode | undefined {
  const added = (context.draft.mutations ?? []).find((mutation) => mutation.op === "add_node" && mutation.node.key === key);
  return added?.op === "add_node" ? added.node : context.tree.byKey.get(key);
}

const LINE_FIELDS = ["id", "title"] as const;
/** Markdown, which keeps its line breaks: a textarea. */
const MARKDOWN_FIELDS = ["description", "prompt", "help"] as const;
const NUMBER_FIELDS = ["weight", "estimate"] as const;
const FLAG_FIELDS = ["placeholder", "requires_artifact", "requires_note", "requires_checklist", "final", "auto_done"] as const;
const MILESTONE_FIELDS = ["feeds_milestone"] as const satisfies readonly ReferenceField[];
const includes = <T extends string>(list: readonly T[], field: string): field is T => (list as readonly string[]).includes(field);

/** A field the proposal sets, edited with authoring's controls for that field; a value with a problem is held here, not written. */
function FieldValue(props: { editor: string; mutation: Extract<Mutation, { op: "set_node_field" }>; onChange: (mutation: Mutation) => void; context: ChangesContext }) {
  const node = nodeOf(props.context, props.mutation.node);
  if (node === undefined) {
    return <span className="muted small">Its node is not in the graph, so its value cannot be edited here.</span>;
  }
  return <FieldValueOf {...props} node={node} />;
}

function FieldValueOf({ editor, mutation, onChange, context, node }: { editor: string; mutation: Extract<Mutation, { op: "set_node_field" }>; onChange: (mutation: Mutation) => void; context: ChangesContext; node: GraphNode }) {
  const field = Object.keys(mutation.value)[0] as NodeField;
  const synced = useSyncedDraft<NodeDraft>(mutation.value, () => draftOf(withFieldValue(node, mutation.value)));
  const draft = synced.draft;
  const kept = useKeptItem();
  const form = useDraftProblems(node.kind, draft);
  const problem = form.problems[field];
  useInvalid(context, editor, problem !== undefined || !form.checked);
  const edited = useRef<NodeDraft | undefined>(undefined);
  const change = (next: Partial<NodeDraft>) => {
    const merged = { ...draft, ...next };
    edited.current = merged;
    synced.set(merged);
  };
  // An edit is written once the engine has read it and found nothing wrong at this field.
  useEffect(() => {
    if (edited.current === draft && form.checked && problem === undefined) {
      edited.current = undefined;
      const value = valueOf(field, draft);
      synced.wrote(value);
      onChange({ ...mutation, value });
    }
  });
  // The items that carry a tick now, from the journey as it is, since the proposal's own graph may already have dropped them: a new item never takes their ids.
  const ticked = context.ready === undefined ? [] : checklistOf(context.ready, node.key).filter((item) => item.ticked).map(({ id, title }) => ({ id, title }));
  const props = { kind: node.kind, nodeKey: node.key, draft, change, notes: () => ({ problem }), tree: context.tree, roles: context.roles, ticked, newItem: newItemOf(kept, change) };
  const targets = includes(MILESTONE_FIELDS, field) ? nodesFor(context.tree, field).map((each) => ({ value: each.key, label: each.title })) : [];
  return (
    <FieldBox label={fieldWords(field)} place={field} notes={{ problem }}>
      <CheckFailed failure={form.failure} retry={form.retry} />
      {includes(LINE_FIELDS, field) ? <Input aria-label={fieldWords(field)} value={draft[field]} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {includes(MARKDOWN_FIELDS, field) ? <textarea aria-label={fieldWords(field)} value={draft[field]} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {includes(NUMBER_FIELDS, field) ? <Input type="number" min={0} aria-label={fieldWords(field)} value={draft[field]} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {includes(FLAG_FIELDS, field) ? <PickBox aria-label={fieldWords(field)} checked={draft[field]} onChange={(event) => { change({ [field]: event.target.checked }); }} /> : null}
      {includes(MILESTONE_FIELDS, field) ? <Picker aria-label={fieldWords(field)} value={draft[field]} none="None" options={targets} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {field === "fills_role" ? <Picker aria-label={fieldWords(field)} value={draft.fills_role} none="No role" options={rolesForFill(context.roles, draft.answer_type).map((role) => ({ value: role.key, label: role.title ?? role.id }))} onChange={(event) => { change({ fills_role: event.target.value }); }} /> : null}
      {field === "parent" ? <ParentValue mutation={mutation} onChange={onChange} context={context} /> : null}
      {field === "choices" ? <ChoicesEditor props={props} /> : null}
      {field === "checklist" ? <ChecklistEditor props={props} flag={false} /> : null}
      {field === "relevant_when" ? <ConditionEditor value={draft.relevant_when} onChange={(next) => { change({ relevant_when: next }); }} tree={context.tree} node={mutation.node} identities={context.deployment.identities ?? []} today={context.today} /> : null}
      {field === "due_by" || field === "not_before" ? <DateRuleEditor which={field} value={draft[field]} onChange={(next) => { change({ [field]: next }); }} tree={context.tree} node={mutation.node} /> : null}
    </FieldBox>
  );
}

/** A2: a move: the container the node goes in, or the top level. */
function ParentValue({ mutation, onChange, context }: { mutation: Extract<Mutation, { op: "set_node_field" }>; onChange: (mutation: Mutation) => void; context: ChangesContext }) {
  const value = "parent" in mutation.value ? (mutation.value.parent ?? "") : "";
  return (
    <Picker aria-label="parent" value={value} none="At the top level" options={moveTargets(context.tree, mutation.node).map((node) => ({ value: node.key, label: `${node.title} (${pathOf(context.tree, node.key)})` }))} onChange={(event) => { onChange({ ...mutation, value: { parent: event.target.value === "" ? null : event.target.value } }); }} />
  );
}

/** The editor a mutation has here, if any. */
function MutationEditor({ editor, mutation, onChange, context }: { editor: string; mutation: Mutation; onChange: (mutation: Mutation) => void; context: ChangesContext }) {
  switch (mutation.op) {
    case "add_node":
      return <AddedNode editor={editor} node={mutation.node} onChange={(node) => { onChange({ ...mutation, node }); }} context={context} />;
    case "set_node_field":
      return <FieldValue editor={editor} mutation={mutation} onChange={onChange} context={context} />;
    case "set_pin":
      return <WhenInput label="Pinned date" value={mutation.date} zone={context.ready?.inputs.timezone ?? "UTC"} onChange={(date) => { onChange({ ...mutation, date }); }} />;
    case "answer": {
      const decision = context.tree.byKey.get(mutation.decision);
      return context.ready === undefined || decision === undefined ? null : (
        <>
          <AnswerInput view={context.ready} node={decision} value={mutation.value} onChange={(value) => { onChange({ ...mutation, value }); }} />
          <label className="stack">
            <span className="label">Why (optional, markdown)</span>
            <textarea value={mutation.rationale ?? ""} onChange={(event) => { onChange({ ...mutation, rationale: event.target.value.trim() === "" ? null : event.target.value }); }} />
          </label>
        </>
      );
    }
    default:
      return null;
  }
}

/** Whether a mutation has an editor here. */
function editable(mutation: Mutation, context: ChangesContext): boolean {
  switch (mutation.op) {
    case "add_node":
    case "set_pin":
      return true;
    case "set_node_field":
      return (NODE_FIELDS as readonly string[]).includes(Object.keys(mutation.value)[0] ?? "");
    case "answer":
      return context.ready !== undefined && context.tree.byKey.has(mutation.decision);
    default:
      return false;
  }
}

function MutationRow({ index, editor, mutation, context }: { index: number; editor: string; mutation: Mutation; context: ChangesContext }) {
  const [open, setOpen] = useState(false);
  // The editor holds an edit the engine has not passed yet (or a value it refused): closing would drop it, so Done waits.
  const holding = context.invalid.has(editor);
  const node = subjectOf(mutation);
  return (
    <li className="stack proposal-change" data-testid="change" data-op={mutation.op} data-node={node}>
      <span className="row">
        <span data-testid="change-words">{mutationWords(mutation, context.names)}</span>
        {context.editable && editable(mutation, context) ? (
          <Button disabled={open && holding} onClick={() => { setOpen(!open); }} aria-expanded={open}>
            {open ? "Done" : "Edit"}
          </Button>
        ) : null}
        {context.editable ? <Button onClick={() => { context.edit(withoutMutation(context.draft, index)); }}>Drop</Button> : null}
      </span>
      {open ? <MutationEditor editor={editor} mutation={mutation} onChange={(next) => { context.edit(withMutation(context.draft, index, next)); }} context={context} /> : null}
    </li>
  );
}

/** B10, A1: a node added to the proposal: its kind, title, and container. */
function AddNode({ context }: { context: ChangesContext }) {
  const pending = (context.draft.mutations ?? []).flatMap((mutation) => (mutation.op === "add_node" ? [mutation.node] : []));
  const lastParent = pending.at(-1)?.parent ?? undefined;
  // What is typed is kept like any unsent form, so switching to another node's editor and back finds it.
  const [kept, setKept] = useDraft<NewNodeValues>(addNodeKey(context.proposal), isNewNodeValues);
  const { kind, title, parent } = kept ?? { kind: "action", title: "", parent: context.defaultParent ?? lastParent ?? "" };
  const check = useNewNodeCheck(title, context.tree.byKey.size);
  const unsendable = !check.ready;
  const add = () => {
    if (unsendable) {
      return;
    }
    const node = newNode(context.tree, kind, title, parent === "" ? undefined : parent, pending);
    context.edit(withMutationsAdded(context.draft, [{ op: "add_node", node }]));
    setKept({ kind, title: "", parent });
  };
  return (
    <form className="stack" data-testid="proposal-add-node" onSubmit={(event) => { event.preventDefault(); add(); }}>
      <NewNodeFields values={{ kind, title, parent }} onChange={setKept} tree={context.tree} check={check} />
      <span>
        <Button type="submit" disabled={unsendable}>
          Add a node
        </Button>
      </span>
    </form>
  );
}

/** A18: a node removed in the proposal, with the rewrites its removal needs. */
function RemoveNode({ context }: { context: ChangesContext }) {
  const [key, setKey] = useState("");
  const plan = useRemovalPlan(context.tree.graph, context.tree, key === "" ? undefined : { node: key });
  return (
    <span className="row" data-testid="proposal-remove-node">
      <Picker aria-label="Remove" value={key} none="Remove a node" options={nodesByPath(context.tree).map((node) => ({ value: node.key, label: `${node.title} (${pathOf(context.tree, node.key)})` }))} onChange={(event) => { setKey(event.target.value); }} />
      {plan === undefined ? null : (
        <>
          <span className="muted small">
            {plan.nodes.length - 1 === 0 ? "Nothing beneath it" : `${String(plan.nodes.length - 1)} beneath it`}
            {plan.dangling.length === 0 ? "" : `; ${String(plan.dangling.length)} references rewritten`}
          </span>
          <Button onClick={() => { context.edit(withMutationsAdded(context.draft, removalMutations(plan))); setKey(""); }}>Remove it</Button>
        </>
      )}
    </span>
  );
}

/** A3: an edge added in the proposal; the proposal's preview refuses one that cannot be drawn. */
function AddEdge({ context }: { context: ChangesContext }) {
  const [node, setNode] = useState("");
  const [requires, setRequires] = useState("");
  const options = nodesByPath(context.tree).map((each) => ({ value: each.key, label: `${each.title} (${pathOf(context.tree, each.key)})` }));
  return (
    <span className="row" data-testid="proposal-add-edge">
      <Picker aria-label="Node" value={node} none="A node" options={options} onChange={(event) => { setNode(event.target.value); }} />
      requires
      <Picker aria-label="Requires" value={requires} none="another" options={options} onChange={(event) => { setRequires(event.target.value); }} />
      <Button disabled={node === "" || requires === ""} onClick={() => { context.edit(withMutationsAdded(context.draft, [{ op: "add_edge", edge: { node, requires } }])); setNode(""); setRequires(""); }}>
        Add the edge
      </Button>
    </span>
  );
}

/** A name for each mutation that stays with it while the list changes around it (model.ts `carryIds`). */
function useCarriedIds(mutations: readonly Mutation[]): string[] {
  const held = useRef<{ mutations: readonly Mutation[]; ids: string[] }>({ mutations: [], ids: [] });
  const next = useRef(0);
  if (held.current.mutations !== mutations) {
    const ids = carryIds(held.current, mutations, () => {
      next.current += 1;
      return `change-${String(next.current)}`;
    });
    held.current = { mutations, ids };
  }
  return held.current.ids;
}

/** What a reviewer can add to the proposal: a node, a removal, an edge. Opens on a node typed and not yet added, so it is never out of sight. */
export function Additions({ context }: { context: ChangesContext }) {
  const typed = useNodeTyped(context.proposal);
  if (!context.editable || !context.hasGraph) {
    return null;
  }
  return (
    <Section title="Add a change" summary={typed ? "a node not yet added" : undefined} open={typed} testId="proposal-additions">
      <AddNode context={context} />
      <RemoveNode context={context} />
      <AddEdge context={context} />
    </Section>
  );
}

/** C14: the proposal's changes, each editable; with `only`, just the changes whose node it keeps. */
export function ChangeList({ context, only }: { context: ChangesContext; only?: (node: string | undefined) => boolean }) {
  const mutations = context.draft.mutations ?? [];
  const ids = useCarriedIds(mutations);
  const kept = mutations.filter((mutation) => only === undefined || only(subjectOf(mutation)));
  if (only !== undefined && kept.length === 0) {
    return null;
  }
  return (
    <div className="stack">
      {mutations.length === 0 ? <p className="muted small">No changes yet.</p> : null}
      <ol className="plain-list stack proposal-changes" data-testid="changes">
        {mutations.map((mutation, index) =>
          only !== undefined && !only(subjectOf(mutation)) ? null : <MutationRow key={ids[index]} editor={ids[index] ?? String(index)} index={index} mutation={mutation} context={context} />,
        )}
      </ol>
    </div>
  );
}
