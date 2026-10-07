// A10: `ResourceEditor` adds, edits, and removes a node's resources: a markdown tip, a link to
// a template, an example, or a reference, or a message draft. A draft's placeholders are
// picked from what the graph holds (journey fields, each role's names, each decision's
// answer) and written as a route file writes them, then stored with keys (Identity and
// references), so renaming a role or moving a decision breaks nothing. Limits and a link's
// form are checked before sending.
import type { Schema } from "@cairn/client";
import { useDraft } from "../data/drafts.ts";

import { Button, Field } from "../ui/kit.tsx";
import type { Resource } from "./graph.ts";
import { mintKey } from "./keys.ts";
import { BODY_BYTES_MAX, TITLE_BYTES_MAX, overBytes, urlProblem } from "./limits.ts";
import { Picker } from "./parts.tsx";
import { placeholderOptions, toStored, toWritten } from "./placeholders.ts";
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
interface ResourceForm {
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

function formOf(resource: Resource, authored: Authored): ResourceForm {
  const type = typeOf(resource);
  const raw = resource[type] ?? "";
  const body = type === "message_draft" ? toWritten(raw, authored.tree, authored.graph.roles ?? []) : raw;
  return { key: resource.key, type, title: resource.title ?? "", body };
}

/** The resource a form makes, or why it cannot be sent. */
export function resourceOf(form: ResourceForm, authored: Authored): { resource: Schema<"Resource"> } | { problem: string } {
  const titleProblem = overBytes(form.title, TITLE_BYTES_MAX, "The title");
  if (titleProblem !== undefined) {
    return { problem: titleProblem };
  }
  if (form.body.trim() === "") {
    return { problem: form.type === "tip" || form.type === "message_draft" ? "It needs some text." : "It needs an address." };
  }
  let content = form.body;
  if (form.type === "message_draft") {
    const stored = toStored(form.body, authored.tree, authored.graph.roles ?? []);
    if ("problem" in stored) {
      return stored;
    }
    content = stored.stored;
  }
  const problem = form.type === "tip" || form.type === "message_draft" ? overBytes(content, BODY_BYTES_MAX, "The text") : urlProblem(content);
  if (problem !== undefined) {
    return { problem };
  }
  const title = form.title.trim();
  return { resource: { key: form.key ?? mintKey("a_"), ...(title === "" ? {} : { title }), [form.type]: content } };
}

function ResourceFields({ form, setForm, authored }: { form: ResourceForm; setForm: (next: ResourceForm) => void; authored: Authored }) {
  const options = placeholderOptions(authored.tree, authored.graph.roles ?? []);
  const multiline = form.type === "tip" || form.type === "message_draft";
  return (
    <>
      <span className="row">
        <Picker aria-label="Resource type" value={form.type} options={TYPES} disabled={form.key !== undefined} onChange={(event) => { setForm({ ...form, type: event.target.value as ResourceType }); }} />
        <Field aria-label="Resource title" placeholder="Title" value={form.title} onChange={(event) => { setForm({ ...form, title: event.target.value }); }} />
      </span>
      {multiline ? (
        <textarea className="textarea" aria-label={form.type === "tip" ? "Tip" : "Message draft"} value={form.body} onChange={(event) => { setForm({ ...form, body: event.target.value }); }} />
      ) : (
        <Field aria-label="Address" placeholder="https://..." value={form.body} onChange={(event) => { setForm({ ...form, body: event.target.value }); }} />
      )}
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

/** One resource's form: a new one, or an edit of one the node has. */
/** A resource's form as kept across reloads, with the revision its author opened it at (H5). */
interface Editing {
  form: ResourceForm;
  base: number;
}

function ResourceFormView({ authored, node, editing, setEditing }: NodeEditorProps & { editing: Editing; setEditing: (next: Editing | undefined) => void }) {
  const write = useAuthorWrite(authored);
  const { form, base } = editing;
  const setForm = (next: ResourceForm) => {
    setEditing({ form: next, base });
  };
  const onClose = () => {
    setEditing(undefined);
  };
  const made = resourceOf(form, authored);
  const save = (from = base) => {
    if ("problem" in made) {
      return;
    }
    if (from !== base) {
      setEditing({ form, base: from });
    }
    const mutation: Schema<"Mutation"> = form.key === undefined ? { op: "add_resource", node: node.key, resource: made.resource } : { op: "edit_resource", node: node.key, resource: made.resource };
    void write.run([mutation], [], from).then((landed) => {
      if (landed) {
        onClose();
      }
    });
  };
  return (
    <div className="stack" data-testid="resource-form">
      <ResourceFields form={form} setForm={setForm} authored={authored} />
      {"problem" in made && form.body !== "" ? <span className="author-problem" role="alert" data-testid="problem">{made.problem}</span> : null}
      <span className="row">
        <Button primary disabled={write.disabled || "problem" in made} onClick={() => { save(); }}>
          {form.key === undefined ? "Add the resource" : "Save the resource"}
        </Button>
        <Button onClick={onClose}>Cancel</Button>
      </span>
      <Refusal write={write} onRetry={() => { save(authored.revision); }} />
    </div>
  );
}

export function ResourceEditor({ authored, node }: NodeEditorProps) {
  const write = useAuthorWrite(authored);
  const [editing, setEditing] = useDraft<Editing>(`resource-form:${domainOf(authored)}:${node.key}`);
  const resources = node.resources ?? [];
  return (
    <div className="stack" data-testid="resource-editor">
      {resources.length === 0 ? <span className="muted">No resources.</span> : null}
      <ul className="detail-list">
        {resources.map((resource) => (
          <li key={resource.key} className="row" data-testid="resource" data-key={resource.key} data-type={typeOf(resource)}>
            <span>
              {resource.title ?? TYPES.find((each) => each.value === typeOf(resource))?.label} <span className="muted">({typeOf(resource).replace("_", " ")})</span>
            </span>
            <Button onClick={() => { setEditing({ form: formOf(resource, authored), base: authored.revision }); }}>Edit</Button>
            <Button aria-label={`Remove the resource ${resource.title ?? resource.key}`} disabled={write.disabled} onClick={() => void write.run([{ op: "remove_resource", node: node.key, resource: resource.key }])}>
              Remove
            </Button>
          </li>
        ))}
      </ul>
      {editing === undefined ? (
        <span className="row">
          <Button onClick={() => { setEditing({ form: { key: undefined, type: "tip", title: "", body: "" }, base: authored.revision }); }}>Add a resource</Button>
        </span>
      ) : (
        <ResourceFormView authored={authored} node={node} editing={editing} setEditing={setEditing} />
      )}
      <Refusal write={write} />
    </div>
  );
}
