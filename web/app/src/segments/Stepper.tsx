// C19, B13: inserting a segment, in the inspector column, three short steps with one primary
// each: choose a segment, place it (under, name, starts after, comes before), and, when the
// segment declares roles or kinds, who they become here. Placing and mapping are previewed by
// the tab's engine as the incoming root and its wiring on the canvas, in review's Add style;
// `Review` drafts one `insert_segment` proposal and opens proposal review (I5: an agent's
// insertion and a person's are reviewed the same way). The same stepper serves a journey and a
// route or segment draft, since both are an `Authored` graph.
import "./segments.css";

import { useEffect, useMemo, useState } from "react";

import { canHoldChildren, nodesByPath, pathOf, type Graph } from "../authoring/graph.ts";
import { mintKey } from "../authoring/keys.ts";
import { Picker } from "../authoring/parts.tsx";
import { domainOf, type Authored } from "../authoring/target.ts";
import { useDraft } from "../data/drafts.ts";
import { Problem, useDraftAndOpen } from "../proposals/Entries.tsx";
import { Button, Field } from "../ui/kit.tsx";
import { stepperDraftKey } from "./entry.ts";
import { insertionOrigins, NO_CHOICES, planInsertion, sizeWords, type Choices, type Mapping, type PeopleRow, type Plan } from "./model.ts";
import { useCandidate, type Candidate } from "./preview.ts";
import { useSegmentNames, useSegmentOffers, useSegmentVersion, type SegmentOffer, type SegmentVersion } from "./read.ts";

/** What the canvas draws while a segment is being placed: the graph before and after, and how its new insertion is named. */
export interface InsertPreview {
  before: Graph;
  after: Graph;
  origins: ReturnType<typeof insertionOrigins>;
}

type Step = 1 | 2 | 3;

/** One segment of the list: its name, a line of what it is, and its size; the latest version is chosen, the others folded. */
function Offer({ offer, onChoose }: { offer: SegmentOffer; onChoose: (version: number) => void }) {
  const older = Array.from({ length: offer.latest - 1 }, (_, at) => offer.latest - 1 - at);
  return (
    <li className="stack segment-offer" data-testid="segment-offer" data-segment={offer.id}>
      <button type="button" className="segment-choice" onClick={() => { onChoose(offer.latest); }}>
        <strong>{offer.name}</strong>
        {offer.description === undefined ? null : <span className="muted small">{offer.description}</span>}
        <span className="muted small">{sizeWords(offer.version.graph)}</span>
      </button>
      {older.length === 0 ? null : (
        <details className="small">
          <summary className="muted">Other versions</summary>
          <span className="row">
            {older.map((version) => (
              <Button key={version} ghost onClick={() => { onChoose(version); }}>
                Use version {version}
              </Button>
            ))}
          </span>
        </details>
      )}
    </li>
  );
}

function ChooseStep({ authored, onChoose }: { authored: Authored; onChoose: (segment: string, version: number) => void }) {
  const { offers, loading, leftOut } = useSegmentOffers("route" in authored.target ? authored.target.route : undefined);
  const [text, setText] = useState("");
  const shown = offers.filter((offer) => `${offer.name} ${offer.description ?? ""}`.toLowerCase().includes(text.trim().toLowerCase()));
  return (
    <div className="stack" data-testid="step-choose">
      <Field aria-label="Search segments" placeholder="Search segments" value={text} onChange={(event) => { setText(event.target.value); }} />
      {loading ? <p className="muted small">Reading the segments...</p> : null}
      {!loading && offers.length === 0 ? <p className="muted small">{leftOut ? "No other segments are published yet." : "There are no published segments yet. Publish one from the Library first."}</p> : null}
      {!loading && offers.length > 0 && shown.length === 0 ? <p className="muted small">No segment is named like that.</p> : null}
      <ul className="stack segment-offers">
        {shown.map((offer) => (
          <Offer key={offer.id} offer={offer} onChoose={(version) => { onChoose(offer.id, version); }} />
        ))}
      </ul>
    </div>
  );
}

/** A list of graph nodes picked one by one: those chosen, each removable, and a picker to add another. */
function NodePicks({ label, authored, chosen, onChange }: { label: string; authored: Authored; chosen: readonly string[]; onChange: (next: string[]) => void }) {
  const options = nodesByPath(authored.tree)
    .filter((node) => !chosen.includes(node.key))
    .map((node) => ({ value: node.key, label: `${node.title} (${pathOf(authored.tree, node.key)})` }));
  return (
    <div className="stack" data-testid="node-picks" data-field={label}>
      <span className="author-label">{label}</span>
      {chosen.map((key) => (
        <span key={key} className="row" data-testid="pick" data-node={key}>
          <span>{authored.tree.byKey.get(key)?.title ?? key}</span>
          <Button ghost aria-label={`Remove ${authored.tree.byKey.get(key)?.title ?? key} from ${label}`} onClick={() => { onChange(chosen.filter((each) => each !== key)); }}>
            ×
          </Button>
        </span>
      ))}
      <Picker aria-label={`Add to ${label}`} value="" none="Add a node" options={options} onChange={(event) => { onChange([...chosen, event.target.value]); }} />
    </div>
  );
}

function PlaceStep({ authored, plan, choices, onChange }: { authored: Authored; plan: Plan; choices: Choices; onChange: (next: Partial<Choices>) => void }) {
  const containers = nodesByPath(authored.tree).filter((node) => canHoldChildren(node.kind));
  return (
    <div className="stack" data-testid="step-place">
      <label className="stack">
        <span className="author-label">Under</span>
        <Picker aria-label="Under" value={choices.parent ?? ""} none="At the top level" options={containers.map((node) => ({ value: node.key, label: `${node.title} (${pathOf(authored.tree, node.key)})` }))} onChange={(event) => { onChange({ parent: event.target.value === "" ? undefined : event.target.value }); }} />
      </label>
      <label className="stack">
        <span className="author-label">Name</span>
        <Field aria-label="Name" value={choices.title} placeholder={plan.root.title} onChange={(event) => { onChange({ title: event.target.value }); }} />
        {plan.root.idTaken ? (
          <span className="muted small" data-testid="id-suffix">
            Its id here is {plan.root.id}: the first is taken.
          </span>
        ) : null}
      </label>
      <NodePicks label="Starts after" authored={authored} chosen={choices.after} onChange={(after) => { onChange({ after }); }} />
      <NodePicks label="Comes before" authored={authored} chosen={choices.before} onChange={(before) => { onChange({ before }); }} />
    </div>
  );
}

function PeopleStep({ rows, where, onMap }: { rows: readonly PeopleRow[]; where: string; onMap: (row: PeopleRow, mapping: Mapping) => void }) {
  return (
    <div className="stack" data-testid="step-people">
      {rows.map((row) => (
        <div key={`${row.type}:${row.key}`} className="stack" data-testid="people-row" data-key={row.key}>
          <label className="stack">
            <span className="author-label">{row.title}</span>
            <Picker
              aria-label={`${row.title} becomes`}
              value={row.mapping === "add" ? "" : row.mapping.existing}
              none={row.type === "role" ? "New role" : "New kind"}
              options={row.options.map((option) => ({ value: option.key, label: `${option.title} (${where})` }))}
              onChange={(event) => { onMap(row, event.target.value === "" ? "add" : { existing: event.target.value }); }}
            />
          </label>
          {row.filled === undefined ? null : (
            <p className="muted small" data-testid="filled-here">
              {row.filled.by === undefined ? "Already filled here." : `Filled here by ${row.filled.by}.`} The segment's {row.filled.leftOut.map((node) => node.title).join(", ")} will be left out.
              <br />
              <button type="button" className="link" onClick={() => { onMap(row, "add"); }}>
                Ask separately
              </button>
            </p>
          )}
        </div>
      ))}
    </div>
  );
}

function Refusal({ candidate }: { candidate: Candidate }) {
  return candidate.status !== "refused" ? null : (
    <ul className="callout callout-bad stack" role="alert" data-testid="insert-refused">
      {candidate.reasons.map((reason, at) => (
        <li key={at}>{reason}</li>
      ))}
    </ul>
  );
}

export interface InsertStepperProps {
  authored: Authored;
  /** The container the root is first placed under (a container's "Insert segment here"). */
  parent: string | undefined;
  onClose: () => void;
  /** Tells the page what to draw: the incoming root as proposed, or nothing while it cannot be placed. */
  onPreview: (preview: InsertPreview | undefined) => void;
}

/** What the stepper keeps of an insertion not yet reviewed: the step, the segment and version, the choices, and the key it will mint. */
interface Held {
  step: Step;
  picked: { id: string; version: number } | undefined;
  choices: Choices;
  insertion: string;
}

/** The stepper's state: kept across a reload, and what it plans and previews as. */
function useInsertion(authored: Authored, parent: string | undefined, onPreview: InsertStepperProps["onPreview"]) {
  const [kept, keep] = useDraft<Held>(stepperDraftKey(domainOf(authored)));
  // Minted once: the preview and the apply write the same keys (B13).
  const [fresh] = useState<Held>(() => ({ step: 1, picked: undefined, choices: { ...NO_CHOICES, parent }, insertion: mintKey("i_") }));
  const { step, picked, choices, insertion } = kept ?? fresh;
  const loaded = useSegmentVersion(picked?.id, picked?.version);
  const nameOf = useSegmentNames();
  const plan = useMemo(
    () => (picked === undefined || loaded === undefined ? undefined : planInsertion(authored.graph, authored.tree, loaded.version.graph, { route: picked.id, version: picked.version }, insertion, choices)),
    [authored, picked, loaded, insertion, choices],
  );
  const candidate = useCandidate(authored, step === 1 ? undefined : plan?.mutation, loaded);
  // The last placed preview stays while the next settles, so the canvas does not flicker between the graph and the proposal.
  useEffect(() => {
    if (step === 1 || candidate.status === "refused") {
      onPreview(undefined);
    } else if (candidate.status === "placed") {
      onPreview({ before: candidate.before, after: candidate.after, origins: insertionOrigins(candidate.before, candidate.after, nameOf) });
    }
  }, [step, candidate, nameOf, onPreview]);
  useEffect(() => () => { onPreview(undefined); }, [onPreview]);
  const held = kept ?? fresh;
  return {
    step,
    setStep: (next: Step) => { keep({ ...held, step: next }); },
    loaded,
    plan,
    candidate,
    choices,
    change: (next: Partial<Choices>) => { keep({ ...held, choices: { ...choices, ...next } }); },
    choose: (id: string, version: number) => { keep({ ...held, step: 2, picked: { id, version }, choices: { ...NO_CHOICES, parent: choices.parent } }); },
  };
}

export function InsertStepper({ authored, parent, onClose, onPreview }: InsertStepperProps) {
  const { step, setStep, loaded, plan, candidate, choices, change, choose } = useInsertion(authored, parent, onPreview);
  const people = plan !== undefined && plan.rows.length > 0;
  const { write, start } = useDraftAndOpen();
  const review = (segment: SegmentVersion, mutation: Plan["mutation"]) => {
    const draft = { title: `Insert ${segment.segment.header.name}`, destination_revision: authored.revision, mutations: [mutation] };
    // Drafted, the insertion is no longer unsent: the stepper and its choices go with it.
    start(["insert", authored.target, authored.revision, mutation], (proposals, patchId, id) =>
      proposals.create(authored.target, { patch_id: patchId, id, draft }).then((written) => {
        if (written.outcome === "answered") {
          onClose();
        }
        return written;
      }),
    );
  };
  const last = step === 3 || (step === 2 && !people);
  return (
    <aside className="detail-panel panel stack insert-stepper" aria-label="Insert a segment" data-testid="insert-stepper" data-step={step}>
      <h2>{step === 1 ? "Choose a segment" : step === 2 ? "Place it" : "People"}</h2>
      {step === 1 || loaded === undefined ? null : (
        <p className="muted small">
          {loaded.segment.header.name}, version {loaded.version.version} · Step {step} of {people ? 3 : 2}
        </p>
      )}
      {step === 1 ? <ChooseStep authored={authored} onChoose={choose} /> : null}
      {step > 1 && (plan === undefined || loaded === undefined) ? <p className="muted small">Reading the segment...</p> : null}
      {step === 2 && plan !== undefined ? <PlaceStep authored={authored} plan={plan} choices={choices} onChange={change} /> : null}
      {step === 3 && plan !== undefined ? (
        <PeopleStep
          rows={plan.rows}
          where={"journey" in authored.target ? "this journey" : "this draft"}
          onMap={(row, mapping) => {
            change(row.type === "role" ? { roles: { ...choices.roles, [row.key]: mapping } } : { kinds: { ...choices.kinds, [row.key]: mapping } });
          }}
        />
      ) : null}
      {step === 1 ? null : <Refusal candidate={candidate} />}
      <Problem problem={write.problem} onDismiss={write.dismiss} />
      <span className="row">
        {step === 1 ? null : (
          <Button ghost onClick={() => { setStep(step === 3 ? 2 : 1); }}>
            Back
          </Button>
        )}
        <span className="spacer" />
        <Button ghost disabled={write.pending} onClick={onClose}>
          Cancel
        </Button>
        {step === 1 ? null : last ? (
          <Button primary disabled={write.disabled || loaded === undefined || plan === undefined || candidate.status !== "placed"} onClick={() => { if (loaded !== undefined && plan !== undefined) { review(loaded, plan.mutation); } }} data-testid="insert-review">
            Review
          </Button>
        ) : (
          <Button primary disabled={plan === undefined} onClick={() => { setStep(3); }} data-testid="insert-next">
            Next
          </Button>
        )}
      </span>
    </aside>
  );
}
