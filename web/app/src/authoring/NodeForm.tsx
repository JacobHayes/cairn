// `NodeForm`: one node's fields, exactly its kind's (A1a), edited together and saved as one
// patch of one-field mutations (an answer-type change replaces the node whole). The draft is
// kept per tab across reloads (ARCHITECTURE, Web UI: drafts survive a reload). What it would
// send is previewed with the wasm engine as the author types, so every violation shows at its
// field before Save (A15); limits stop a value before it is sent (PRACTICES); and a journey's
// route-copied fields carry their local-edit marker (B4).
import { useDraft } from "../data/drafts.ts";
import { Button } from "../ui/kit.tsx";
import { ConditionEditor } from "./ConditionEditor.tsx";
import { conditionProblem } from "./condition.ts";
import { DateRuleEditor } from "./DateRuleEditor.tsx";
import { partsOf, ruleProblem } from "./dates.ts";
import { changesOf, draftOf, draftProblems, type FormField, type NodeDraft } from "./fields.ts";
import type { GraphNode } from "./graph.ts";
import { DecisionFields, FieldGroup, SharedFields, StageFields, WorkFields, has, type FieldsProps } from "./NodeFields.tsx";
import { FieldBox, FormViolations, StaleRejection, type FieldNotes } from "./parts.tsx";
import { domainOf, type Authored } from "./target.ts";
import { byPlace, type Place, type Violation } from "./violations.ts";
import { useAuthorWrite, useLivePreview, type AuthorWrite } from "./write.ts";

interface Kept {
  /** Only the fields the author changed, so the rest follow the node live (H5). */
  edits: Partial<NodeDraft>;
  /** The revision the author started from (H5). */
  base: number;
}

/** Every problem that stops the draft being sent, by field: limits, and a condition's or rule's own. */
export function problemsOf(node: GraphNode, draft: NodeDraft): Partial<Record<FormField, string>> {
  const problems = draftProblems(node.kind, draft);
  const condition = draft.relevant_when === null ? undefined : conditionProblem(draft.relevant_when);
  if (condition !== undefined) {
    problems.relevant_when = condition;
  }
  for (const which of ["due_by", "not_before"] as const) {
    const rule = draft[which];
    const problem = rule === null ? undefined : ruleProblem(partsOf(rule));
    if (problem !== undefined) {
      problems[which] = problem;
    }
  }
  return problems;
}

/** Whether a local-edit marker names `field` of the node (B4). */
function editedFields(authored: Authored, node: string): Set<string> {
  const markers = authored.graph.state?.local_edits?.[node] ?? [];
  return new Set(markers.flatMap((marker) => (typeof marker === "object" && "field" in marker ? [marker.field] : [])));
}

/** The violations the form shows: the live preview's, or the last attempt's when it is about these fields. */
function shownViolations(preview: ReturnType<typeof useLivePreview>, write: AuthorWrite): { violations: Violation[]; places: Place[] } {
  if (preview?.outcome === "rejected" && preview.rejection.rejection === "invalid") {
    return { violations: preview.rejection.violations, places: [] };
  }
  const failed = write.failed;
  if (failed?.rejection.rejection === "invalid") {
    return { violations: failed.rejection.violations, places: failed.sent.places };
  }
  return { violations: [], places: [] };
}

function Relevance({ props }: { props: FieldsProps & { authored: Authored } }) {
  return (
    <FieldBox label="Relevant when" place="relevant_when" notes={props.notes("relevant_when")}>
      <ConditionEditor
        value={props.draft.relevant_when}
        onChange={(next) => { props.change({ relevant_when: next }); }}
        tree={props.tree}
        node={props.nodeKey}
        entities={props.authored.deployment.entities ?? []}
        today={props.authored.today}
      />
    </FieldBox>
  );
}

function Rules({ props }: { props: FieldsProps }) {
  return (
    <>
      {(["due_by", "not_before"] as const).map((which) => (
        <FieldBox key={which} label={which === "due_by" ? "Due by (finishes by)" : "Not before (starts no sooner than)"} place={which} notes={props.notes(which)}>
          <DateRuleEditor which={which} value={props.draft[which]} onChange={(next) => { props.change({ [which]: next }); }} tree={props.tree} node={props.nodeKey} />
        </FieldBox>
      ))}
    </>
  );
}

function Fields({ props }: { props: FieldsProps & { authored: Authored } }) {
  return (
    <>
      <FieldGroup title="What it is">
        <SharedFields props={props} />
        {props.kind === "decision" ? <DecisionFields props={props} /> : <WorkFields props={props} />}
        {has(props, "opens_at") ? <StageFields props={props} /> : null}
      </FieldGroup>
      <FieldGroup title="When it applies">
        <Relevance props={props} />
      </FieldGroup>
      <FieldGroup title="Dates">
        <Rules props={props} />
      </FieldGroup>
    </>
  );
}

export interface NodeFormProps {
  authored: Authored;
  node: GraphNode;
}

export function NodeForm({ authored, node }: NodeFormProps) {
  const write = useAuthorWrite(authored);
  const [kept, setKept] = useDraft<Kept>(`node-form:${domainOf(authored)}:${node.key}`);
  const edits = kept?.edits ?? {};
  const draft: NodeDraft = { ...draftOf(node), ...edits };
  const changes = changesOf(node, draft, new Set(Object.keys(edits) as FormField[]));
  const problems = problemsOf(node, draft);
  const sendable = changes.length > 0 && Object.keys(problems).length === 0;
  const preview = useLivePreview(authored, sendable ? changes.map((change) => change.mutation) : undefined);
  const { violations, places: sent } = shownViolations(preview, write);
  const places = byPlace(violations, node.key, sent);
  const edited = editedFields(authored, node.key);
  const notes = (field: FormField): FieldNotes => ({ problem: problems[field], violations: places.get(field), edited: edited.has(field) });
  const change = (next: Partial<NodeDraft>) => {
    setKept({ edits: { ...edits, ...next }, base: kept?.base ?? authored.revision });
    write.dismiss();
  };
  const save = async (base = kept?.base) => {
    if (await write.run(changes.map((each) => each.mutation), changes.map((each) => each.field), base)) {
      setKept(undefined);
    }
  };
  /** Drops the unsent change and its rejection. */
  const discard = () => {
    setKept(undefined);
    write.dismiss();
  };
  const props = { kind: node.kind, nodeKey: node.key, draft, change, notes, tree: authored.tree, roles: authored.graph.roles ?? [], authored };
  return (
    <form className="stack" data-testid="node-form" data-node={node.key} onSubmit={(event) => { event.preventDefault(); void save(); }}>
      <Fields props={props} />
      <FormViolations places={places} />
      {write.failed === undefined ? null : <StaleRejection rejection={write.failed.rejection} onRetry={() => void save(authored.revision)} onDismiss={discard} />}
      <span className="row">
        <Button type="submit" primary disabled={write.disabled || !sendable} data-testid="node-form-save">
          Save {changes.length === 0 ? "" : `(${String(changes.length)} ${changes.length === 1 ? "change" : "changes"})`}
        </Button>
        <Button disabled={kept === undefined} onClick={discard}>
          Discard changes
        </Button>
        {preview?.outcome === "accepted" ? <span className="muted small" data-testid="preview" data-status="accepted">The engine accepts this.</span> : null}
        {preview?.outcome === "rejected" ? <span className="muted small" data-testid="preview" data-status="rejected">The engine would refuse this; see the fields.</span> : null}
        {preview?.outcome === "unavailable" ? <span className="author-problem" data-testid="preview" data-status="unavailable">No preview: {preview.message}</span> : null}
      </span>
    </form>
  );
}
