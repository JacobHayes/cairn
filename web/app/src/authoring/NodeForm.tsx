// `NodeForm`: one node's fields, exactly its kind's (A1a), edited together and saved as one
// patch of one-field mutations (an answer-type change replaces the node whole). The draft is
// kept per tab across reloads (ARCHITECTURE, Web UI: drafts survive a reload). What it would
// send is previewed with the wasm engine as the author types, so every violation shows at its
// field before Save (A15); limits stop a value before it is sent (PRACTICES); and a journey's
// route-copied fields carry their local-edit marker (B4).
import { useDraft } from "../data/drafts.ts";
import { Failed } from "../ui/Failed.tsx";
import { Button } from "../ui/kit.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { ConditionEditor } from "./ConditionEditor.tsx";
import { useConditionProblem } from "./condition-problem.ts";
import { DateRuleEditor } from "./DateRuleEditor.tsx";
import { partsOf, ruleProblem } from "./dates.ts";
import { changesOf, draftOf, draftProblems, type FormField, type NodeDraft } from "./fields.ts";
import { Section } from "../detail/parts.tsx";
import type { GraphNode } from "./graph.ts";
import { BasicsFields, CompletionFields, DecisionFields, StageFields, WeightFields, has, typedItem, type FieldsProps } from "./NodeFields.tsx";
import type { Schema } from "@cairn/client";
import type { Violation } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { FieldBox, FormViolations, type FieldNotes } from "./parts.tsx";
import { domainOf, type Authored } from "./target.ts";
import { byPlace, type Place } from "./violations.ts";
import { useAuthorWrite, useLivePreview, type AuthorWrite } from "./write.ts";
import { plural } from "../ui/words.ts";

interface Kept {
  /** Only the fields the author changed, so the rest follow the node live (H5). */
  edits: Partial<NodeDraft>;
  /** The revision the author started from (H5). */
  base: number;
  /** Text typed in the checklist's new-item field and not yet added, so a reload keeps it. */
  newItem?: string;
}

/** Every problem that stops the draft being sent, by field: limits, and a rule's own. A condition's is the engine's, read by `useConditionProblem`. */
export function problemsOf(node: GraphNode, draft: NodeDraft): Partial<Record<FormField, string>> {
  const problems = draftProblems(node.kind, draft);
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
function shownViolations(preview: ReturnType<typeof useLivePreview>, write: AuthorWrite): { violations: Violation[]; places: (Place | undefined)[] } {
  if (preview?.outcome === "rejected" && preview.rejection.rejection === "invalid") {
    return { violations: preview.rejection.violations, places: [] };
  }
  const failed = write.failed;
  if (failed?.rejection.rejection === "invalid") {
    return { violations: failed.rejection.violations, places: failed.sent.places };
  }
  return { violations: [], places: [] };
}

/** G5: the node's saved checklist items that carry a tick, whose ids a new item must not take, so it never inherits the tick. */
function ticked(authored: Authored, node: GraphNode): Schema<"ChecklistItem">[] {
  const marked = new Set(authored.graph.state?.ticks?.[node.key] ?? []);
  return (node.checklist ?? []).filter((item) => marked.has(item.id));
}

function Relevance({ props }: { props: FieldsProps & { authored: Authored } }) {
  return (
    <FieldBox label="Relevant when" place="relevant_when" notes={props.notes("relevant_when")}>
      <ConditionEditor
        value={props.draft.relevant_when}
        onChange={(next) => { props.change({ relevant_when: next }); }}
        tree={props.tree}
        node={props.nodeKey}
        identities={props.authored.deployment.identities ?? []}
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

/** A section's fields hold a violation or a problem: it opens so the author sees where. */
function holds(props: FieldsProps, fields: readonly FormField[]): boolean {
  return fields.some((field) => {
    const notes = props.notes(field);
    return notes.problem !== undefined || (notes.violations ?? []).length > 0;
  });
}

/**
 * A9, A8, F4, A16: the form's fields in sections that open by kind (design 4.9): Basics always,
 * Question for a decision, and the rest folded under their names, each opening itself when one
 * of its fields has a violation so it shows at its field (A15).
 */
function Fields({ props }: { props: FieldsProps & { authored: Authored } }) {
  const completion = (["checklist", "requires_checklist", "requires_artifact", "requires_note", "placeholder"] as const).filter((field) => has(props, field));
  return (
    <>
      <Section title="Basics" open testId="form-basics">
        <BasicsFields props={props} />
      </Section>
      {props.kind === "decision" ? (
        <Section title="Question" open testId="form-question">
          <DecisionFields props={props} />
        </Section>
      ) : null}
      <Section title="Relevance" open={holds(props, ["relevant_when"])} testId="form-relevance">
        <Relevance props={props} />
      </Section>
      <Section title="Dates" open={holds(props, ["due_by", "not_before", "opens_at", "closes_at"])} testId="form-dates">
        <Rules props={props} />
        {has(props, "opens_at") ? <StageFields props={props} /> : null}
      </Section>
      {completion.length === 0 ? null : (
        <Section title="Completion needs" open={holds(props, completion)} testId="form-completion">
          <CompletionFields props={props} />
        </Section>
      )}
      <Section title="Weight and estimate" open={holds(props, ["weight", "estimate"])} testId="form-weight">
        <WeightFields props={props} />
      </Section>
    </>
  );
}

export interface NodeFormProps {
  authored: Authored;
  node: GraphNode;
}

export function NodeForm({ authored, node }: NodeFormProps) {
  const write = useAuthorWrite(authored, `node-form:${node.key}`, node.title);
  const [kept, setKept] = useDraft<Kept>(`node-form:${domainOf(authored)}:${node.key}`);
  const edits = kept?.edits ?? {};
  const draft: NodeDraft = { ...draftOf(node), ...edits };
  // Text typed in the new-item field and not yet added goes with Save as an item, so one press saves it (the field itself still shows it until it is added).
  const typed = typedItem(draft.checklist, ticked(authored, node).map((item) => item.id), kept?.newItem ?? "");
  const sending: NodeDraft = typed === undefined || !has({ kind: node.kind, draft }, "checklist") ? draft : { ...draft, checklist: [...draft.checklist, typed] };
  const changes = changesOf(node, sending, new Set([...Object.keys(edits), ...(sending === draft ? [] : ["checklist"])] as FormField[]));
  const condition = useConditionProblem(draft.relevant_when);
  const problems = { ...problemsOf(node, sending), ...(condition.problem === undefined ? {} : { relevant_when: condition.problem }) };
  const sendable = changes.length > 0 && condition.checked && Object.keys(problems).length === 0;
  const preview = useLivePreview(authored, sendable ? changes.map((change) => change.mutation) : undefined);
  const { violations, places: sent } = shownViolations(preview, write);
  const places = byPlace(violations, node.key, sent);
  const edited = editedFields(authored, node.key);
  const notes = (field: FormField): FieldNotes => ({ problem: problems[field], violations: places.get(field), edited: edited.has(field) });
  const change = (next: Partial<NodeDraft>) => {
    setKept({ ...kept, edits: { ...edits, ...next }, base: kept?.base ?? authored.revision });
    write.dismiss();
  };
  const newItem = {
    value: kept?.newItem ?? "",
    set: (value: string) => {
      setKept({ edits, base: kept?.base ?? authored.revision, ...(value === "" ? {} : { newItem: value }) });
    },
    add: (next: Partial<NodeDraft>) => {
      setKept({ edits: { ...edits, ...next }, base: kept?.base ?? authored.revision });
      write.dismiss();
    },
  };
  const save = async (base = kept?.base) => {
    if (await write.run(changes.map((each) => each.mutation), base)) {
      setKept(undefined);
    }
  };
  /** Drops the unsent change and its rejection. */
  const discard = () => {
    setKept(undefined);
    write.dismiss();
  };
  const props = { kind: node.kind, nodeKey: node.key, draft, change, notes, tree: authored.tree, roles: authored.graph.roles ?? [], authored, ticked: ticked(authored, node), newItem };
  return (
    <form className="stack" data-testid="node-form" data-node={node.key} onSubmit={(event) => { event.preventDefault(); void save(); }}>
      <div className="detail-folds">
        <Fields props={props} />
      </div>
      <FormViolations places={places} />
      <WriteStatus view={write.status} />
      {write.failed === undefined ? <Receipt receipt={write.receipt} /> : write.failed.rejection.rejection === "invalid" ? null : <RejectionView rejection={write.failed.rejection} onRebase={() => void save(authored.revision)} onDismiss={discard} />}
      {kept === undefined && changes.length === 0 ? null : (
        <span className="row">
          <Button type="submit" primary disabled={write.disabled || !sendable} data-testid="node-form-save" onMouseDown={(event) => { event.preventDefault(); }}>
            Save {changes.length === 0 ? "" : `(${plural(changes.length, "change")})`}
          </Button>
          <Button disabled={kept === undefined} onClick={discard}>
            Discard changes
          </Button>
          {preview?.outcome === "accepted" ? <span className="muted small" data-testid="preview" data-status="accepted">Ready to save.</span> : null}
          {preview?.outcome === "rejected" ? <span className="muted small" data-testid="preview" data-status="rejected">This would be refused; see the fields.</span> : null}
          {preview?.outcome === "unavailable" ? <span className="author-problem" data-testid="preview" data-status="unavailable"><Failed what="The preview" verb="made" error={preview.message} /></span> : null}
        </span>
      )}
    </form>
  );
}
