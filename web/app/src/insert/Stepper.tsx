// C19, B13: inserting a route, in the inspector column, three short steps with one primary
// each: choose a route, place it (under, name, starts after, comes before), and, when the
// route declares roles or kinds, who they become here. Placing and mapping are previewed by
// the tab's engine as the incoming root and its wiring on the canvas, in review's Add style;
// `Review` drafts one `insert_route` proposal and opens proposal review (I5: an agent's
// insertion and a person's are reviewed the same way). The same stepper serves a journey and a
// route draft, since both are an `Authored` graph.
import "./insert.css";

import { useActorRef, useSelector } from "@xstate/react";
import { useEffect, useMemo, useState } from "react";

import { canHoldChildren, type Graph, nodesByPath, pathOf, titleIn } from "../authoring/graph.ts";
import { Picker } from "../authoring/parts.tsx";
import { domainOf, type Authored } from "../authoring/target.ts";
import { readDraft, writeDraft } from "../data/drafts.ts";
import { Problem, useDraftAndOpen } from "../proposals/Entries.tsx";
import { Button, Input } from "../ui/kit.tsx";
import { stepperDraftKey } from "./entry.ts";
import { heldOf, insertion, stepOf } from "./insertion.ts";
import { insertionMembers, planInsertion, type Choices, type Mapping, type PeopleRow, type Plan } from "./model.ts";
import { useCandidate, type Candidate } from "./preview.ts";
import { useInsertionPlan, useRouteNames, useRouteOffers, useRouteCopy, type RouteOffer, type RouteCopy } from "./read.ts";

/** What the canvas draws while a route is being placed: the graph before and after, and how its new insertion is named. */
export interface InsertPreview {
  before: Graph;
  after: Graph;
  members: Record<string, string>;
}

/** One route of the list: its name and a line of what it is; the latest version is chosen, the others folded. */
function Offer({ offer, onChoose }: { offer: RouteOffer; onChoose: (version: number) => void }) {
  const older = Array.from({ length: offer.latest - 1 }, (_, at) => offer.latest - 1 - at);
  return (
    <li className="stack route-offer" data-testid="route-offer" data-route={offer.id}>
      <button type="button" className="route-choice" onClick={() => { onChoose(offer.latest); }}>
        <strong>{offer.name}</strong>
        {offer.description === undefined ? null : <span className="muted small">{offer.description}</span>}
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

function ChooseStep({ onChoose }: { onChoose: (route: string, version: number) => void }) {
  const { offers, loading } = useRouteOffers();
  const [text, setText] = useState("");
  const shown = offers.filter((offer) => `${offer.name} ${offer.description ?? ""}`.toLowerCase().includes(text.trim().toLowerCase()));
  return (
    <div className="stack" data-testid="step-choose">
      <Input aria-label="Search routes" placeholder="Search routes" value={text} onChange={(event) => { setText(event.target.value); }} />
      {loading ? <p className="muted small">Reading the routes...</p> : null}
      {!loading && offers.length === 0 ? <p className="muted small">There are no published routes yet.</p> : null}
      <ul className="plain-list stack route-offers">
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
          <span>{titleIn(authored.tree, key)}</span>
          <Button ghost aria-label={`Remove ${titleIn(authored.tree, key)} from ${label}`} onClick={() => { onChange(chosen.filter((each) => each !== key)); }}>
            Remove
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
      {plan.root === undefined ? null : (
        <label className="stack">
          <span className="author-label">Name</span>
          <Input aria-label="Name" value={choices.title} placeholder={plan.root.title} onChange={(event) => { onChange({ title: event.target.value }); }} />
        </label>
      )}
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
              {row.filled.by === undefined ? "Already filled here." : `Filled here by ${row.filled.by}.`} The route's {row.filled.leftOut.map((node) => node.title).join(", ")} will be left out.
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
  /** The container the root is first placed under (a container's "Insert route here"). */
  parent: string | undefined;
  onClose: () => void;
  /** Tells the page what to draw: the incoming root as proposed, or nothing while it cannot be placed. */
  onPreview: (preview: InsertPreview | undefined) => void;
}

/** The stepper's state: kept across a reload, and what it plans and previews as. */
function useInsertion(authored: Authored, parent: string | undefined, onPreview: InsertStepperProps["onPreview"]) {
  const draftKey = stepperDraftKey(domainOf(authored));
  // Corrected on the way in, once: the machine is the draft's owner from then on.
  const [held] = useState(() => heldOf(readDraft(draftKey), authored.tree, parent));
  const flow = useActorRef(insertion, { input: { held, keep: (next) => { writeDraft(draftKey, next); } } });
  const step = useSelector(flow, stepOf);
  const { picked, choices, insertion: key } = useSelector(flow, (snapshot) => snapshot.context);
  const loaded = useRouteCopy(picked?.id, picked?.version);
  const nameOf = useRouteNames();
  const engine = useInsertionPlan(authored.graph, loaded?.version.graph, { roles: choices.roles, kinds: choices.kinds });
  const plan = useMemo(
    () => (picked === undefined || loaded === undefined || engine === undefined ? undefined : planInsertion(authored.graph, loaded.version.graph, engine.plan, { route: picked.id, version: picked.version }, key, choices)),
    [authored, picked, loaded, engine?.plan, key, choices],
  );
  // Whether the route has people to map is known once its plan is: a people step restored from a reload waits for that.
  const people = plan !== undefined && plan.rows.length > 0;
  useEffect(() => {
    if (plan !== undefined) {
      flow.send({ type: "planned", people });
    }
  }, [flow, plan === undefined, people]);
  const candidate = useCandidate(authored, step === 1 ? undefined : plan?.mutation, loaded);
  // The last placed preview stays while the next settles, so the canvas does not flicker between the graph and the proposal.
  useEffect(() => {
    if (step === 1 || candidate.status === "refused") {
      onPreview(undefined);
    } else if (candidate.status === "placed") {
      onPreview({ before: candidate.before, after: candidate.after, members: insertionMembers(candidate.before, candidate.after, nameOf) });
    }
  }, [step, candidate, nameOf, onPreview]);
  useEffect(() => () => { onPreview(undefined); }, [onPreview]);
  return {
    step,
    loaded,
    plan,
    settled: engine?.current === true,
    people,
    candidate,
    choices,
    next: () => { flow.send({ type: "next", people }); },
    back: () => { flow.send({ type: "back" }); },
    change: (next: Partial<Choices>) => { flow.send({ type: "change", next }); },
    choose: (id: string, version: number) => { flow.send({ type: "choose", id, version }); },
  };
}

export function InsertStepper({ authored, parent, onClose, onPreview }: InsertStepperProps) {
  const { step, loaded, plan, settled, people, candidate, choices, next, back, change, choose } = useInsertion(authored, parent, onPreview);
  const { write, start } = useDraftAndOpen(`insert:${domainOf(authored)}`);
  const review = (copy: RouteCopy, mutation: Plan["mutation"]) => {
    const draft = { title: `Insert ${copy.route.header.name}`, destination_revision: authored.revision, mutations: [mutation] };
    // Drafted, the insertion is no longer unsent: the stepper and its choices go with it.
    start((id) => ({ op: "create", destination: authored.target, request: { id, draft } }), onClose);
  };
  const last = step === 3 || (step === 2 && !people);
  return (
    <aside className="detail-panel panel stack insert-stepper" aria-label="Insert a route" data-testid="insert-stepper" data-step={step}>
      <h2>{step === 1 ? "Choose a route" : step === 2 ? "Place it" : "People"}</h2>
      {step === 1 || loaded === undefined ? null : (
        <p className="muted small">
          {loaded.route.header.name}, version {loaded.version.version} · Step {step} of {people ? 3 : 2}
        </p>
      )}
      {step === 1 ? <ChooseStep onChoose={choose} /> : null}
      {step > 1 && (plan === undefined || loaded === undefined) ? <p className="muted small">Reading the route...</p> : null}
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
      <Problem write={write} onRebase={write.dismiss} />
      <span className="row">
        {step === 1 ? null : (
          <Button ghost onClick={back}>
            Back
          </Button>
        )}
        <span className="spacer" />
        <Button ghost disabled={write.pending} onClick={onClose}>
          Cancel
        </Button>
        {step === 1 ? null : last ? (
          <Button primary disabled={write.disabled || !settled || loaded === undefined || plan === undefined || candidate.status !== "placed"} onClick={() => { if (loaded !== undefined && plan !== undefined) { review(loaded, plan.mutation); } }} data-testid="insert-review">
            Review
          </Button>
        ) : (
          <Button primary disabled={plan === undefined || !settled} onClick={next} data-testid="insert-next">
            Next
          </Button>
        )}
      </span>
    </aside>
  );
}
