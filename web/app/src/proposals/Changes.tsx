// C14: a proposal's changes, editable one by one: each mutation in words with its own editor
// where it has one (a node it adds with 5.6's field editors, a field it sets with the
// condition or date-rule editor, a pin's date, an answer with 5.1's answer input) and a way to
// drop it; and what a reviewer adds: a node (B10: a breakdown's pieces), a node removed with
// its cascade (A18), and an edge (A3). Nothing is sent from here: the edits change the draft,
// previewed at once, and are saved with the proposal.
import { useEffect, useRef, useState } from "react";

import { ConditionEditor } from "../authoring/ConditionEditor.tsx";
import { planRemoval, removalMutations } from "../authoring/cascade.ts";
import { DateRuleEditor } from "../authoring/DateRuleEditor.tsx";
import { draftOf, draftProblems, nodeFrom, valueOf, type FormField, type NodeDraft } from "../authoring/fields.ts";
import { KINDS, canHoldChildren, childrenOf, edgeRefusal, moveTargets, nodesByPath, pathOf, type GraphNode, type Mutation, type NodeField, type NodeKind, type Role, type Tree } from "../authoring/graph.ts";
import { mintKey, slugOf, uniqueId } from "../authoring/keys.ts";
import { ChoicesEditor, DecisionFields, FieldGroup, SharedFields, StageFields, WorkFields, has } from "../authoring/NodeFields.tsx";
import { problemsOf } from "../authoring/NodeForm.tsx";
import { FieldBox, Picker } from "../authoring/parts.tsx";
import type { Deployment } from "../data/host.ts";
import { Input as AnswerInput } from "../detail/AnswerEditor.tsx";
import type { Ready } from "../detail/model.ts";
import { Button, Field } from "../ui/kit.tsx";
import { carryIds, withFieldValue, withMutation, withMutationsAdded, withoutMutation, type ProposalDraft } from "./model.ts";
import { fieldName, mutationNode, mutationWords, type Names } from "./words.ts";

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
  selected: string | undefined;
  /** Where a new node goes unless another is picked (a breakdown's placeholder). */
  defaultParent: string | undefined;
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

/** A node the proposal adds, edited with authoring's fields; the mutation follows once nothing stops it. */
function AddedNode({ editor, node, onChange, context }: { editor: string; node: GraphNode; onChange: (node: GraphNode) => void; context: ChangesContext }) {
  const synced = useSyncedDraft<NodeDraft>(node, () => draftOf(node));
  const draft = synced.draft;
  const problems = problemsOf(node, draft);
  useInvalid(context, editor, Object.keys(problems).length > 0);
  const change = (next: Partial<NodeDraft>) => {
    const merged = { ...draft, ...next };
    synced.set(merged);
    if (Object.keys(problemsOf(node, merged)).length === 0) {
      const written = nodeFrom(node, merged);
      synced.wrote(written);
      onChange(written);
    }
  };
  const props = {
    kind: node.kind,
    nodeKey: node.key,
    draft,
    change,
    notes: (field: FormField) => ({ problem: problems[field] }),
    tree: context.tree,
    roles: context.roles,
  };
  return (
    <div className="stack" data-testid="added-node-form">
      <FieldGroup title="What it is">
        <SharedFields props={props} />
        {node.kind === "decision" ? <DecisionFields props={props} /> : <WorkFields props={props} />}
        {has(props, "opens_at") ? <StageFields props={props} /> : null}
      </FieldGroup>
      <FieldGroup title="When it applies and its dates">
        <ConditionEditor value={draft.relevant_when} onChange={(next) => { change({ relevant_when: next }); }} tree={context.tree} node={node.key} entities={context.deployment.entities ?? []} today={context.today} />
        {(["due_by", "not_before"] as const).map((which) => (
          <DateRuleEditor key={which} which={which} value={draft[which]} onChange={(next) => { change({ [which]: next }); }} tree={context.tree} node={node.key} />
        ))}
      </FieldGroup>
    </div>
  );
}

/** The node a set field is on, as the proposal leaves it: one it adds, or the graph's. */
function nodeOf(context: ChangesContext, key: string): GraphNode | undefined {
  const added = (context.draft.mutations ?? []).find((mutation) => mutation.op === "add_node" && mutation.node.key === key);
  return added?.op === "add_node" ? added.node : context.tree.byKey.get(key);
}

/** The fields a set field's editor offers here: every node field (A1a). */
export const EDITABLE_FIELDS: ReadonlySet<string> = new Set(["id", "title", "description", "prompt", "help", "weight", "estimate", "placeholder", "requires_artifact", "final", "auto_reach", "gates", "closes", "parent", "opens_at", "closes_at", "feeds_milestone", "fills_role", "choices", "relevant_when", "due_by", "not_before"]);

const LINE_FIELDS = ["id", "title"] as const;
/** Markdown, which keeps its line breaks: a textarea. */
const MARKDOWN_FIELDS = ["description", "prompt", "help"] as const;
const NUMBER_FIELDS = ["weight", "estimate"] as const;
const FLAG_FIELDS = ["placeholder", "requires_artifact", "final", "auto_reach", "gates", "closes"] as const;
const MILESTONE_FIELDS = ["opens_at", "closes_at", "feeds_milestone"] as const;
const includes = <T extends string>(list: readonly T[], field: string): field is T => (list as readonly string[]).includes(field);

/** A field the proposal sets, edited with authoring's controls for that field; a value with a problem is held here, not written. */
function FieldValue({ editor, mutation, onChange, context }: { editor: string; mutation: Extract<Mutation, { op: "set_node_field" }>; onChange: (mutation: Mutation) => void; context: ChangesContext }) {
  const field = Object.keys(mutation.value)[0] as NodeField;
  const node = nodeOf(context, mutation.node);
  const synced = useSyncedDraft<NodeDraft | undefined>(mutation.value, () => (node === undefined ? undefined : draftOf(withFieldValue(node, mutation.value))));
  const draft = synced.draft;
  const problem = node === undefined || draft === undefined ? undefined : draftProblems(node.kind, draft)[field];
  useInvalid(context, editor, problem !== undefined);
  if (node === undefined || draft === undefined) {
    return <span className="muted">Its node is not in the graph, so its value cannot be edited here.</span>;
  }
  const change = (next: Partial<NodeDraft>) => {
    const merged = { ...draft, ...next };
    synced.set(merged);
    if (draftProblems(node.kind, merged)[field] === undefined) {
      const value = valueOf(field, merged);
      synced.wrote(value);
      onChange({ ...mutation, value });
    }
  };
  const props = { kind: node.kind, nodeKey: node.key, draft, change, notes: () => ({ problem }), tree: context.tree, roles: context.roles };
  const milestones = nodesByPath(context.tree, "milestone").map((each) => ({ value: each.key, label: each.title }));
  return (
    <FieldBox label={fieldName(field)} place={field} notes={{ problem }}>
      {includes(LINE_FIELDS, field) ? <Field aria-label={fieldName(field)} value={draft[field]} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {includes(MARKDOWN_FIELDS, field) ? <textarea className="textarea" aria-label={fieldName(field)} value={draft[field]} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {includes(NUMBER_FIELDS, field) ? <Field type="number" min={0} aria-label={fieldName(field)} value={draft[field]} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {includes(FLAG_FIELDS, field) ? <input type="checkbox" aria-label={fieldName(field)} checked={draft[field]} onChange={(event) => { change({ [field]: event.target.checked }); }} /> : null}
      {includes(MILESTONE_FIELDS, field) ? <Picker aria-label={fieldName(field)} value={draft[field]} none="None" options={milestones} onChange={(event) => { change({ [field]: event.target.value }); }} /> : null}
      {field === "fills_role" ? <Picker aria-label={fieldName(field)} value={draft.fills_role} none="No role" options={context.roles.map((role) => ({ value: role.key, label: role.title ?? role.id }))} onChange={(event) => { change({ fills_role: event.target.value }); }} /> : null}
      {field === "parent" ? <ParentValue mutation={mutation} onChange={onChange} context={context} /> : null}
      {field === "choices" ? <ChoicesEditor props={props} /> : null}
      {field === "relevant_when" ? <ConditionEditor value={draft.relevant_when} onChange={(next) => { change({ relevant_when: next }); }} tree={context.tree} node={mutation.node} entities={context.deployment.entities ?? []} today={context.today} /> : null}
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
      return <Field type="date" aria-label="Pinned date" value={mutation.date} onChange={(event) => { onChange({ ...mutation, date: event.target.value }); }} />;
    case "answer": {
      const decision = context.tree.byKey.get(mutation.decision);
      return context.ready === undefined || decision === undefined ? null : (
        <>
          <AnswerInput view={context.ready} node={decision} value={mutation.value} onChange={(value) => { onChange({ ...mutation, value }); }} />
          <textarea className="textarea" aria-label="Why" placeholder="Why (optional, markdown)" value={mutation.rationale ?? ""} onChange={(event) => { onChange({ ...mutation, rationale: event.target.value.trim() === "" ? null : event.target.value }); }} />
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
      return EDITABLE_FIELDS.has(Object.keys(mutation.value)[0] ?? "");
    case "answer":
      return context.ready !== undefined && context.tree.byKey.has(mutation.decision);
    default:
      return false;
  }
}

function MutationRow({ index, editor, mutation, context }: { index: number; editor: string; mutation: Mutation; context: ChangesContext }) {
  const [open, setOpen] = useState(false);
  const node = mutationNode(mutation);
  const picked = node !== undefined && node === context.selected;
  return (
    <li className={picked ? "stack proposal-change proposal-picked" : "stack proposal-change"} data-testid="change" data-op={mutation.op} data-node={node}>
      <span className="row">
        <span data-testid="change-words">{mutationWords(mutation, context.names)}</span>
        {context.editable && editable(mutation, context) ? (
          <Button onClick={() => { setOpen(!open); }} aria-expanded={open}>
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
  const [kind, setKind] = useState<NodeKind>("action");
  const [title, setTitle] = useState("");
  const [parent, setParent] = useState(context.defaultParent ?? lastParent ?? "");
  const containers = nodesByPath(context.tree).filter((node) => canHoldChildren(node.kind));
  const add = () => {
    const under = parent === "" ? undefined : parent;
    const taken = new Set([...childrenOf(context.tree, under).map((child) => child.id), ...pending.filter((node) => (node.parent ?? undefined) === under).map((node) => node.id)]);
    const base: GraphNode = { key: mintKey("n_"), id: uniqueId(slugOf(title), taken), kind, title: title.trim() };
    const placed = under === undefined ? base : { ...base, parent: under };
    const node = kind === "decision" ? { ...placed, prompt: title.trim(), answer_type: "boolean" as const } : placed;
    context.edit(withMutationsAdded(context.draft, [{ op: "add_node", node }]));
    setTitle("");
  };
  return (
    <form className="row" data-testid="proposal-add-node" onSubmit={(event) => { event.preventDefault(); add(); }}>
      <Picker aria-label="Kind" value={kind} options={KINDS.map((each) => ({ value: each, label: each }))} onChange={(event) => { setKind(event.target.value as NodeKind); }} />
      <Field aria-label="New node title" placeholder="Title" value={title} onChange={(event) => { setTitle(event.target.value); }} />
      <Picker aria-label="Inside" value={parent} none="At the top level" options={containers.map((node) => ({ value: node.key, label: `${node.title} (${pathOf(context.tree, node.key)})` }))} onChange={(event) => { setParent(event.target.value); }} />
      <Button type="submit" disabled={title.trim() === ""}>
        Add a node
      </Button>
    </form>
  );
}

/** A18: a node removed in the proposal, with the rewrites its removal needs. */
function RemoveNode({ context }: { context: ChangesContext }) {
  const [key, setKey] = useState("");
  const plan = key === "" ? undefined : planRemoval(context.tree, key);
  return (
    <span className="row" data-testid="proposal-remove-node">
      <Picker aria-label="Remove" value={key} none="Remove a node" options={nodesByPath(context.tree).map((node) => ({ value: node.key, label: `${node.title} (${pathOf(context.tree, node.key)})` }))} onChange={(event) => { setKey(event.target.value); }} />
      {plan === undefined ? null : (
        <>
          <span className="muted">
            {plan.nodes.length - 1 === 0 ? "Nothing beneath it" : `${String(plan.nodes.length - 1)} beneath it`}
            {plan.dangling.length === 0 ? "" : `; ${String(plan.dangling.length)} references rewritten`}
          </span>
          <Button onClick={() => { context.edit(withMutationsAdded(context.draft, removalMutations(plan))); setKey(""); }}>Remove it</Button>
        </>
      )}
    </span>
  );
}

/** A3: an edge added in the proposal, refused before it is added when it cannot be drawn. */
function AddEdge({ context }: { context: ChangesContext }) {
  const [node, setNode] = useState("");
  const [requires, setRequires] = useState("");
  const options = nodesByPath(context.tree).map((each) => ({ value: each.key, label: `${each.title} (${pathOf(context.tree, each.key)})` }));
  const refusal = node === "" || requires === "" ? undefined : edgeRefusal(context.tree, node, requires);
  return (
    <span className="row" data-testid="proposal-add-edge">
      <Picker aria-label="Node" value={node} none="A node" options={options} onChange={(event) => { setNode(event.target.value); }} />
      requires
      <Picker aria-label="Requires" value={requires} none="another" options={options} onChange={(event) => { setRequires(event.target.value); }} />
      <Button disabled={node === "" || requires === "" || refusal !== undefined} onClick={() => { context.edit(withMutationsAdded(context.draft, [{ op: "add_edge", edge: { node, requires } }])); setNode(""); setRequires(""); }}>
        Add the edge
      </Button>
      {refusal === undefined ? null : <span className="author-problem">{refusal}</span>}
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

/** C14: the proposal's changes, each editable, and what a reviewer can add. */
export function ChangeList({ context }: { context: ChangesContext }) {
  const mutations = context.draft.mutations ?? [];
  const ids = useCarriedIds(mutations);
  return (
    <div className="stack">
      {mutations.length === 0 ? <p className="muted">No changes yet.</p> : null}
      <ol className="stack proposal-changes" data-testid="changes">
        {mutations.map((mutation, index) => (
          <MutationRow key={ids[index]} editor={ids[index] ?? String(index)} index={index} mutation={mutation} context={context} />
        ))}
      </ol>
      {context.editable ? (
        <div className="stack" data-testid="proposal-additions">
          <AddNode context={context} />
          <RemoveNode context={context} />
          <AddEdge context={context} />
        </div>
      ) : null}
    </div>
  );
}
