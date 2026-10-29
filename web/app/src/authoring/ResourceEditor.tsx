// A10: `ResourceEditor` adds, edits, and removes a node's resources: a markdown tip, a link to
// a template, an example, or a reference, or a message draft. A draft's placeholders are
// picked from what the graph holds (journey fields, each role's names, each decision's
// answer) and written as a route file writes them, then stored with keys (Keys and
// references), so renaming a role or moving a decision breaks nothing. The engine checks
// a resource's values (limits, a link's form) before it is sent.
import type { Schema } from "@cairn/client";
import { useDraft } from "../data/drafts.ts";
import { useId, useState } from "react";
import { useSession } from "../data/react.ts";

import type { Deriver } from "../data/host.ts";
import { Button, Input } from "../ui/kit.tsx";
import type { Resource } from "./graph.ts";
import { mintKey } from "./keys.ts";
import { countCheck, useValueProblems } from "./form-problems.ts";
import { isEditing, unsent, type Editing } from "./editing.ts";
import { CheckFailed, invalidAttrs, Labelled, Picker, Problem } from "./parts.tsx";
import { placeholderOptions, useDraftStored } from "./placeholders.ts";
import { Refusal, type NodeEditorProps } from "./StructureEditors.tsx";
import { domainOf, type Authored } from "./target.ts";
import { useAuthorWrite } from "./write.ts";

/** The kinds of resource (A10), as the schema names their content. */
export type ResourceType = "tip" | "template" | "example" | "reference" | "message_draft";

const TYPES: { value: ResourceType; label: string }[] = [
  { value: "tip", label: "Tip" },
  { value: "template", label: "Template link" },
  { value: "example", label: "Example link" },
  { value: "reference", label: "Reference link" },
  { value: "message_draft", label: "Message draft" },
];

/** A resource as its form holds it. */
export interface ResourceForm {
  key: string | undefined;
  type: ResourceType;
  title: string;
  /** The tip's text, the link's address, or the draft as written (roles by id, decisions by path). */
  body: string;
}

/** A resource's type and its content. */
export function typeOf(resource: Resource): ResourceType {
  return resource.tip !== undefined ? "tip" : resource.message_draft !== undefined ? "message_draft" : resource.template !== undefined ? "template" : resource.example !== undefined ? "example" : "reference";
}

/** A resource's form; a draft's text is shown as an author writes it (the engine's reading of it). */
async function formOf(resource: Resource, authored: Authored, deriver: Deriver): Promise<ResourceForm> {
  const type = typeOf(resource);
  const raw = resource[type] ?? "";
  const body = type === "message_draft" ? await deriver.draftWritten(authored.graph, raw) : raw;
  return { key: resource.key, type, title: resource.title ?? "", body };
}

/** The resource a form makes, once its content is settled (a draft's is the engine's reading of it). */
function resourceFrom(form: ResourceForm, content: string): Schema<"Resource"> {
  const title = form.title.trim();
  return { key: form.key ?? mintKey("a_"), ...(title === "" ? {} : { title }), [form.type]: content };
}

type ControlAttrs = ReturnType<typeof invalidAttrs>;

function ResourceFields({ form, setForm, authored, titleAttrs, contentAttrs }: { form: ResourceForm; setForm: (next: ResourceForm) => void; authored: Authored; titleAttrs: ControlAttrs; contentAttrs: ControlAttrs }) {
  const options = placeholderOptions(authored.tree, authored.graph.roles ?? []);
  const multiline = form.type === "tip" || form.type === "message_draft";
  return (
    <>
      <span className="row">
        <Labelled label="Type"><Picker aria-label="Resource type" value={form.type} options={TYPES} disabled={form.key !== undefined} onChange={(event) => { setForm({ ...form, type: event.target.value as ResourceType }); }} /></Labelled>
        <Labelled label="Title (optional)"><Input aria-label="Resource title" placeholder="Title" {...titleAttrs} value={form.title} onChange={(event) => { setForm({ ...form, title: event.target.value }); }} /></Labelled>
      </span>
      <Labelled label={form.type === "tip" ? "Tip" : form.type === "message_draft" ? "Message draft" : "Address"}>
        {multiline ? (
          <textarea aria-label={form.type === "tip" ? "Tip" : "Message draft"} {...contentAttrs} value={form.body} onChange={(event) => { setForm({ ...form, body: event.target.value }); }} />
        ) : (
          <Input aria-label="Address" placeholder="https://..." {...contentAttrs} value={form.body} onChange={(event) => { setForm({ ...form, body: event.target.value }); }} />
        )}
      </Labelled>
      {form.type === "message_draft" ? (
        <Picker
          aria-label="Insert a placeholder"
          value=""
          none="Insert a placeholder..."
          options={options.map((option) => ({ value: option.written, label: option.label }))}
          onChange={(event) => { setForm({ ...form, body: `${form.body}${event.target.value}` }); }}
        />
      ) : null}
    </>
  );
}

function ResourceFormView({ authored, node, editing, setEditing }: NodeEditorProps & { editing: Editing<ResourceForm>; setEditing: (next: Editing<ResourceForm> | undefined) => void }) {
  const write = useAuthorWrite(authored, `resource-form:${node.key}`, node.title);
  const { value: form, base, opened } = editing;
  const setForm = (next: ResourceForm) => {
    setEditing({ value: next, base, opened });
  };
  const onClose = () => {
    setEditing(undefined);
  };
  const stored = useDraftStored(authored.graph, form.type === "message_draft" ? form.body : undefined);
  const multiline = form.type === "tip" || form.type === "message_draft";
  // A draft is checked as the engine stores it; the rest as typed.
  const content = form.type !== "message_draft" ? form.body : stored.result !== undefined && "stored" in stored.result ? stored.result.stored : undefined;
  // A draft's own reading (the engine's placeholders) owns its validity; a tip's text and a link's address are read as the types they are.
  const checks = useValueProblems([
    { field: "title", kind: "title", what: "The title", value: form.title.trim(), optional: true },
    ...(form.type === "message_draft" ? [] : [{ field: "content", kind: multiline ? ("markdown" as const) : ("url" as const), what: multiline ? "The text" : "The address", value: form.body }]),
  ]);
  const [titleMessage, contentMessage] = [useId(), useId()];
  const titleProblem = checks.problems.title;
  const contentProblem = form.body.trim() === "" ? undefined : stored.result !== undefined && "problem" in stored.result ? stored.result.problem : checks.problems.content;
  const failure = checks.failure ?? stored.failure;
  const made = content === undefined || form.body.trim() === "" || !checks.checked || !stored.checked || titleProblem !== undefined || checks.problems.content !== undefined ? undefined : resourceFrom(form, content);
  const save = (from = base) => {
    if (made === undefined) {
      return;
    }
    if (from !== base) {
      setEditing({ value: form, base: from, opened });
    }
    const mutation: Schema<"Mutation"> = form.key === undefined ? { op: "add_resource", node: node.key, resource: made } : { op: "edit_resource", node: node.key, resource: made };
    void write.run([mutation], from).then((landed) => {
      if (landed) {
        onClose();
      }
    });
  };
  return (
    <div className="stack" data-testid="resource-form">
      <ResourceFields form={form} setForm={setForm} authored={authored} titleAttrs={invalidAttrs(titleMessage, titleProblem)} contentAttrs={invalidAttrs(contentMessage, contentProblem)} />
      {titleProblem === undefined ? null : <Problem id={titleMessage}>{titleProblem}</Problem>}
      {contentProblem === undefined ? null : <Problem id={contentMessage}>{contentProblem}</Problem>}
      <CheckFailed failure={failure} retry={() => { checks.retry(); stored.retry(); }} />
      <span className="row">
        <Button primary disabled={write.disabled || made === undefined} onClick={() => { save(); }}>
          {form.key === undefined ? "Add the resource" : "Save the resource"}
        </Button>
        <Button onClick={onClose}>Cancel</Button>
      </span>
      <Refusal write={write} onRetry={() => { save(authored.revision); }} />
    </div>
  );
}

export function ResourceEditor({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored, `resources:${node.key}`, node.title);
  const { deriver } = useSession();
  // A draft is read before its form opens; nothing else opens one meanwhile, so a late answer cannot replace it.
  const [opening, setOpening] = useState(false);
  const [editing, setEditing] = useDraft<Editing<ResourceForm>>(`resource-form:${domainOf(authored)}:${node.key}`, isEditing);
  const resources = node.resources ?? [];
  const room = useValueProblems([countCheck("count", "resource_count_per_node_max", "resources on a node", resources.length + 1)]);
  const full = room.problems.count !== undefined;
  return (
    <div className="stack" data-testid="resource-editor">
      {resources.length === 0 ? <span className="muted small">No resources.</span> : null}
      <ul className="detail-list">
        {resources.map((resource) => (
          <li key={resource.key} className="row" data-testid="resource" data-key={resource.key} data-type={typeOf(resource)}>
            <span>
              {resource.title ?? TYPES.find((each) => each.value === typeOf(resource))?.label} <span className="muted small">({typeOf(resource).replace("_", " ")})</span>
            </span>
            <Button disabled={opening} held={editing !== undefined && unsent(editing) ? "Save or cancel the form below first, or its changes are lost" : undefined} onClick={() => { setOpening(true); void formOf(resource, authored, deriver).then((form) => { setEditing({ value: form, base: authored.revision, opened: form }); }).finally(() => { setOpening(false); }); }}>Edit</Button>
            <Button aria-label={`Remove the resource ${resource.title ?? resource.key}`} disabled={write.disabled} onClick={() => void write.run([{ op: "remove_resource", node: node.key, resource: resource.key }]).then((landed) => { if (landed && editing?.value.key === resource.key) { setEditing(undefined); } })}>
              Remove
            </Button>
          </li>
        ))}
      </ul>
      {editing === undefined ? (
        <span className="row">
          <Button disabled={full || opening} onClick={() => { const blank: ResourceForm = { key: undefined, type: "tip", title: "", body: "" }; setEditing({ value: blank, base: authored.revision, opened: blank }); }}>Add a resource</Button>
          {room.problems.count === undefined ? null : <span className="muted small">{room.problems.count}</span>}
        </span>
      ) : (
        <ResourceFormView authored={authored} node={node} editing={editing} setEditing={setEditing} />
      )}
      <Refusal write={write} />
    </div>
  );
}
