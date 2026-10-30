// `NodeForm`: one node's fields, exactly its kind's (A1a), edited together and saved as one
// patch of one-field mutations (an answer-type change replaces the node whole). The draft is
// kept per tab across reloads (ARCHITECTURE, Web UI: drafts survive a reload). What it would
// send is previewed with the wasm engine as the author types, so every violation shows at its
// field before Save (A15); the engine's checks of its values stop one before it is sent (PRACTICES), and a check that cannot be made says so and can be asked again.
import { useEffect } from "react";

import { Check } from "../acting/Controls.tsx";
import { useDraft } from "../data/drafts.ts";
import { Failed } from "../ui/Failed.tsx";
import { Button } from "../ui/kit.tsx";
import { WriteStatus } from "../screens/WriteStatus.tsx";
import { Receipt } from "../ui/Receipt.tsx";
import { ConditionEditor } from "./ConditionEditor.tsx";
import { DateRuleEditor } from "./DateRuleEditor.tsx";
import { changesOf, draftOf, type FormField, type NodeDraft } from "./fields.ts";
import { useDraftProblems } from "./form-problems.ts";
import { Section } from "../detail/parts.tsx";
import type { GraphNode } from "./graph.ts";
import { BasicsFields, CompletionFields, DecisionFields, WeightFields, has, typedItem, type FieldsProps } from "./NodeFields.tsx";
import type { Schema } from "@cairn/client";
import type { Violation } from "../data/writes.ts";
import { RejectionView } from "../screens/RejectionView.tsx";
import { CheckFailed, FieldBox, FormViolations, type FieldNotes } from "./parts.tsx";
import { domainOf, type Authored } from "./target.ts";
import { byPlace, type Place } from "./violations.ts";
import { useAuthorWrite, useLivePreview, type AuthorWrite } from "./write.ts";
import { plural } from "../ui/words.ts";

export interface Kept {
  /** Only the fields the author changed, so the rest follow the node live (H5). */
  edits: Partial<NodeDraft>;
  /** The revision the author started from (H5). */
  base: number;
  /** Text typed in the checklist's new-item field and not yet added, so a reload keeps it. */
  newItem?: string;
}

/** Whether a restored draft is a `Kept` the form reads now: its date rules are the editor's parts (an older build kept the engine's rule shape), so one that is not is discarded. */
export function isKept(draft: unknown): draft is Kept {
  if (typeof draft !== "object" || draft === null || !("edits" in draft) || typeof draft.edits !== "object" || draft.edits === null) {
    return false;
  }
  const edits: Record<string, unknown> = { ...draft.edits };
  return [edits["due_by"], edits["not_before"]].every((rule) => rule === undefined || rule === null || (typeof rule === "object" && "sources" in rule && Array.isArray(rule.sources)));
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
 * A9, A8, F4, A16: the form's fields in sections that open by kind: Basics always,
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
      <Section title="Date rules" open={holds(props, ["due_by", "not_before"])} testId="form-dates">
        <Rules props={props} />
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

/** Calls `drop` whenever `idle` holds. */
function useDropped(idle: boolean, drop: () => void): void {
  useEffect(() => {
    if (idle) {
      drop();
    }
  });
}

export function NodeForm({ authored, node }: NodeFormProps) {
  const write = useAuthorWrite(authored, `node-form:${node.key}`, node.title);
  const [kept, setKept] = useDraft<Kept>(`node-form:${domainOf(authored)}:${node.key}`, isKept);
  const edits = kept?.edits ?? {};
  const draft: NodeDraft = { ...draftOf(node), ...edits };
  // Text typed in the new-item field and not yet added goes with Save as an item, so one press saves it (the field itself still shows it until it is added).
  const typed = typedItem(draft.checklist, ticked(authored, node).map((item) => item.id), kept?.newItem ?? "");
  const sending: NodeDraft = typed === undefined || !has({ kind: node.kind, draft }, "checklist") ? draft : { ...draft, checklist: [...draft.checklist, typed] };
  const changes = changesOf(node, sending, new Set([...Object.keys(edits), ...(sending === draft ? [] : ["checklist"])] as FormField[]));
  const form = useDraftProblems(node.kind, sending);
  const { checked, problems } = form;
  const sendable = changes.length > 0 && checked && Object.keys(problems).length === 0;
  const preview = useLivePreview(authored, sendable ? changes.map((change) => change.mutation) : undefined);
  const { violations, places: sent } = shownViolations(preview, write);
  const places = byPlace(violations, node.key, sent);
  const notes = (field: FormField): FieldNotes => ({ problem: problems[field], violations: places.get(field) });
  // A draft that says nothing the node does not already say (what was typed was typed back) is no draft: nothing is left unsaved.
  useDropped(kept !== undefined && changes.length === 0 && (kept.newItem ?? "").trim() === "", () => { setKept(undefined); });
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
  const [proposing = false, setProposing] = useDraft<boolean>(`propose:${domainOf(authored)}:${node.key}`); // kept with the unsent form: a stale save sent again is still a proposal, never a live write
  const save = async (base = kept?.base) => {
    if (await write.run(changes.map((each) => each.mutation), base, proposing ? `Edit ${node.title}` : undefined)) { setKept(undefined); setProposing(undefined); }
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
      <CheckFailed failure={form.failure} retry={form.retry} />
      <WriteStatus view={write.status} />
      {write.failed === undefined ? <Receipt receipt={write.receipt} /> : write.failed.rejection.rejection === "invalid" ? null : <RejectionView rejection={write.failed.rejection} onRebase={() => void save(authored.revision)} onDismiss={discard} />}
      {kept === undefined && changes.length === 0 ? null : (
        <span className="row">
          <Button type="submit" primary disabled={write.disabled || !sendable} data-testid="node-form-save" onMouseDown={(event) => { event.preventDefault(); }}>
            {proposing ? "Propose" : "Save"} {changes.length === 0 ? "" : `(${plural(changes.length, "change")})`}
          </Button>
          <Check label="Propose instead" checked={proposing} onChange={(next) => { setProposing(next ? true : undefined); }} testId="propose-instead" />
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
