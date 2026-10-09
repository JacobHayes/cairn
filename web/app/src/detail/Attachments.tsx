// G1, G2: the node's notes and links, attributed and timestamped, each editable and
// deletable, and a link designated as the node's artifact or not (G2: a `requires_artifact`
// deliverable's guard looks for one on the node itself; removing it after completion leaves
// the node done and `stale`). Adding one is a form kept as a draft until it is sent. The
// journey's own notes and links (G1, on the journey's overview) are the same list with no
// node, and no artifact.
import type { Schema } from "@cairn/client";

import { Badge, Button, Field } from "../ui/kit.tsx";
import { Markdown, safeHref } from "../ui/markdown.tsx";
import type { Annotation, NodeDetail, Ready } from "./model.ts";
import { Section } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite } from "./write.ts";

type Body = Schema<"AnnotationBody">;

/** An annotation's type: a note, or a typed link (G1). */
export type AnnotationType = "note" | "artifact" | "reference" | "conversation";

export const ANNOTATION_TYPES: AnnotationType[] = ["note", "artifact", "reference", "conversation"];

/** What an annotation says: its type and its text or address. */
export function contentOf(body: Body): { type: AnnotationType; text: string } {
  for (const type of ANNOTATION_TYPES) {
    const text = body[type];
    if (text !== undefined) {
      return { type, text };
    }
  }
  return { type: "note", text: "" };
}

/** The body with `type` and `text` as its content, its key, node, and title kept. */
export function withContent(body: Body, type: AnnotationType, text: string): Body {
  const kept: Body = { key: body.key, node: body.node ?? null };
  const titled = body.title == null || body.title === "" ? kept : { ...kept, title: body.title };
  return { ...titled, [type]: text };
}

/** G2: designating a link as the artifact, or a designated one back to a plain reference. */
export function designated(body: Body, artifact: boolean): Body {
  return withContent(body, artifact ? "artifact" : "reference", contentOf(body).text);
}

/** A fresh attachment key: `a_` and 24 hex digits from the browser's random UUID. */
export function newAttachmentKey(): string {
  return `a_${crypto.randomUUID().replaceAll("-", "").slice(0, 24)}`;
}

interface AnnotationDraft {
  key: string;
  /** A new one, rather than an edit of the one with this key. */
  adding: boolean;
  type: AnnotationType;
  title: string;
  text: string;
}

function Editor({ write, node, form, types }: { write: NodeWrite; node: string | null; form: ReturnType<typeof useFormDraft<AnnotationDraft>>; types: AnnotationType[] }) {
  const { draft } = form;
  if (draft === undefined) {
    return null;
  }
  const { value } = draft;
  const body: Body = withContent({ key: value.key, node, ...(value.title === "" ? {} : { title: value.title }) }, value.type, value.text);
  const save = async () => {
    const op = value.adding ? "add_annotation" : "edit_annotation";
    if (await write.run([{ op, annotation: body }], draft)) {
      form.close();
    }
  };
  return (
    <div className="stack" data-testid="annotation-editor">
      <span className="row">
        <select className="select" aria-label="Type" value={value.type} onChange={(event) => { form.change({ ...value, type: event.target.value as AnnotationType }); }}>
          {types.map((type) => (
            <option key={type} value={type}>
              {type === "note" ? "Note" : `${type[0]?.toUpperCase() ?? ""}${type.slice(1)} link`}
            </option>
          ))}
        </select>
        <Field aria-label="Title" placeholder="Title (optional)" value={value.title} onChange={(event) => { form.change({ ...value, title: event.target.value }); }} />
      </span>
      {value.type === "note" ? (
        <textarea className="textarea" aria-label="Note" value={value.text} onChange={(event) => { form.change({ ...value, text: event.target.value }); }} />
      ) : (
        <Field type="url" aria-label="Address" placeholder="https://" value={value.text} onChange={(event) => { form.change({ ...value, text: event.target.value }); }} />
      )}
      <span className="row">
        <Button primary disabled={write.disabled || value.text.trim() === ""} onClick={() => void save()}>
          {value.adding ? "Add" : "Save"}
        </Button>
        <Button onClick={() => { form.close(); write.dismiss(); }}>Cancel</Button>
      </span>
    </div>
  );
}

function Item({ annotation, write, onEdit }: { annotation: Annotation; write: NodeWrite; onEdit: () => void }) {
  const { body } = annotation;
  const { type, text } = contentOf(body);
  const link = type !== "note";
  const designates = link && body.node != null;
  return (
    <li className="stack attachment" data-testid="annotation" data-key={body.key} data-type={type}>
      <span className="row">
        <Badge tone={type === "artifact" ? "good" : "plain"}>{type}</Badge>
        {body.title == null ? null : <strong>{body.title}</strong>}
        {link ? safeHref(text) ? <a href={text} target="_blank" rel="noreferrer noopener">{text}</a> : <span className="mono">{text}</span> : null}
      </span>
      {link ? null : <Markdown text={text} />}
      <span className="muted">
        Added by {annotation.created_by} at {annotation.created_at}
        {annotation.edited_at == null ? "" : `; edited at ${annotation.edited_at}`}
      </span>
      <span className="row">
        <Button disabled={write.disabled} onClick={onEdit}>Edit</Button>
        {designates ? (
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "edit_annotation", annotation: designated(body, type !== "artifact") }])}>
            {type === "artifact" ? "Not the artifact" : "Make it the artifact"}
          </Button>
        ) : null}
        <Button disabled={write.disabled} onClick={() => void write.run([{ op: "remove_annotation", annotation: body.key }])}>
          Remove
        </Button>
      </span>
    </li>
  );
}

/**
 * G1: notes and links on `node`, or on the journey itself when `node` is null, with the form
 * that adds or edits one; `summary` heads the section.
 */
export function AnnotationList({ view, node, annotations, summary }: { view: Ready; node: string | null; annotations: Annotation[]; summary: string }) {
  const write = useNodeWrite(view, `annotations:${node ?? "journey"}`);
  const form = useFormDraft<AnnotationDraft>(write.journey, node ?? "journey", "annotation");
  const types = node === null ? ANNOTATION_TYPES.filter((type) => type !== "artifact") : ANNOTATION_TYPES;
  const edit = (annotation: Annotation) => {
    const { type, text } = contentOf(annotation.body);
    form.open({ key: annotation.body.key, adding: false, type, title: annotation.body.title ?? "", text }, write.seen);
  };
  return (
    <Section title="Notes and links" summary={summary} open testId="annotations">
      {annotations.length === 0 ? <span className="muted">None yet.</span> : null}
      <ul className="checklist stack">
        {annotations.map((annotation) => (
          <Item key={annotation.body.key} annotation={annotation} write={write} onEdit={() => { edit(annotation); }} />
        ))}
      </ul>
      {form.draft === undefined ? (
        <span className="row">
          <Button disabled={write.disabled} onClick={() => { form.open({ key: newAttachmentKey(), adding: true, type: "note", title: "", text: "" }, write.seen); }}>
            Add a note or link
          </Button>
        </span>
      ) : (
        <Editor write={write} node={node} form={form} types={types} />
      )}
      <Rejected view={view} write={write} onResolved={form.close} />
    </Section>
  );
}

/** G1, G2: the node's notes and links, with the form that adds or edits one. */
export function AttachmentList({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const { annotations } = detail;
  const artifacts = annotations.filter((annotation) => annotation.body.artifact !== undefined).length;
  const summary = [
    String(annotations.length),
    detail.node.requires_artifact === true ? `artifact required, ${String(artifacts)} designated` : undefined,
    detail.node.requires_note === true ? "note required" : undefined,
  ];
  return <AnnotationList view={view} node={detail.node.key} annotations={annotations} summary={summary.filter((part) => part !== undefined).join(", ")} />;
}
