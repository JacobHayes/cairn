// The node's journey state a section edits in place (B5, B6, E3, F5): its pin, routed
// through the decision that feeds it when one does; its snooze; a force include with its
// reason; its weight; and its participations. Each is one patch whose rejection shows here,
// a pin's with the moves that resolve a contradictory chain.
import type { Schema } from "@cairn/client";
import type { ReactNode } from "react";

import { Button, Field } from "../ui/kit.tsx";
import { isTerminal, nodeOf, transition, type Mutation, type NodeDetail, type Ready } from "./model.ts";
import { NodeLink } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { entityName } from "./sections.tsx";
import { useFormDraft, useNodeWrite, type NodeWrite, type Seen } from "./write.ts";

type Draft = ReturnType<typeof useFormDraft<string>>;

/** A one-field form: opened with a starting value, kept as a draft, sent as one patch. */
function DraftForm({
  write,
  draft,
  form,
  start,
  label,
  input,
  mutations,
}: {
  write: NodeWrite;
  draft: Draft;
  form: string;
  start: string;
  label: string;
  input: (value: string, change: (value: string) => void) => ReactNode;
  mutations: (value: string) => Mutation[];
}) {
  if (draft.draft === undefined) {
    return (
      <Button disabled={write.disabled} onClick={() => { draft.open(start, write.seen); }}>
        {label}
      </Button>
    );
  }
  const seen = draft.draft;
  const { value } = seen;
  const save = async () => {
    if (await write.run(mutations(value), seen)) {
      draft.close();
    }
  };
  return (
    <span className="row" data-testid={`${form}-form`}>
      {input(value, draft.change)}
      <Button primary disabled={write.disabled || value.trim() === ""} onClick={() => void save()}>
        Save
      </Button>
      <Button onClick={() => { draft.close(); write.dismiss(); }}>Cancel</Button>
    </span>
  );
}

const dateInput = (label: string) => (value: string, change: (value: string) => void) => (
  <Field type="date" aria-label={label} value={value} onChange={(event) => { change(event.target.value); }} />
);

const textInput = (label: string) => (value: string, change: (value: string) => void) => (
  <Field aria-label={label} placeholder={label} value={value} onChange={(event) => { change(event.target.value); }} />
);

/**
 * E3: a decision's answer while it is in effect: decided and relevant (crates/engine
 * `answer_in_effect`). A decision out of scope or undecided keeps its answer, but what it
 * drove (a role, a milestone's pin) is empty.
 */
export function answerInEffect(view: Ready, decision: string): Schema<"AnswerValue"> | undefined {
  const decided = view.journey.graph.state?.nodes?.[decision]?.state === "decided";
  const relevant = view.derived.nodes[decision]?.relevance.value === "relevant";
  return decided && relevant ? view.journey.graph.state?.answers?.[decision] : undefined;
}

/**
 * F2, E3: the node's pin. A milestone a date decision feeds is pinned only through that
 * decision: setting its date answers the decision, and clearing it reopens the decision.
 */
export function PinEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `pin:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const pinForm = useFormDraft<string>(write.journey, key, "pin");
  const fedBy = detail.fedBy;
  const fedAnswer = fedBy === undefined ? undefined : answerInEffect(view, fedBy.key);
  const current = fedBy === undefined ? detail.pin : fedAnswer !== undefined && "date" in fedAnswer ? fedAnswer.date : undefined;
  const set = (date: string): Mutation[] =>
    fedBy === undefined ? [{ op: "set_pin", node: key, date }] : [{ op: "answer", decision: fedBy.key, value: { date } }];
  const clear: Mutation = fedBy === undefined ? { op: "clear_pin", node: key } : transition(fedBy.key, "reopen");
  return (
    <div className="stack" data-testid="pin">
      <span>
        Pin: <strong data-testid="pin-date">{current ?? "none"}</strong>
        {fedBy === undefined ? null : (
          <span className="muted small" data-testid="pin-through">
            {" "}
            set by answering <NodeLink view={view} node={fedBy.key} />: editing it here answers that decision
          </span>
        )}
      </span>
      <span className="row">
        <DraftForm
          write={write}
          draft={pinForm}
          form="pin"
          start={current ?? view.key.today}
          label={current === undefined ? "Pin a date" : "Change the pin"}
          input={dateInput("Pin date")}
          mutations={set}
        />
        {current === undefined ? null : (
          <Button disabled={write.disabled} onClick={() => void write.run([clear])}>
            {fedBy === undefined ? "Unpin" : "Unpin (reopens the decision)"}
          </Button>
        )}
      </span>
      <Rejected view={view} write={write} onResolved={pinForm.close} />
    </div>
  );
}

/** What a snooze waits for: its date, or the node. */
function SnoozeUntil({ view, target }: { view: Ready; target: Schema<"SnoozeTarget"> }) {
  return "date" in target ? target.date : <NodeLink view={view} node={target.node} />;
}

/** B6: a descendant held by its container's snooze, which is the one to lift (the node has no snooze of its own to). */
function SnoozedThrough({ view, write, container, target }: { view: Ready; write: NodeWrite; container: string; target: Schema<"SnoozeTarget"> }) {
  return (
    <span className="row" data-testid="snoozed-via" data-via={container}>
      <span>
        Snoozed through <NodeLink view={view} node={container} />, until <SnoozeUntil view={view} target={target} />
      </span>
      <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node: container }])}>
        Unsnooze {nodeOf(view, container)?.title ?? container}
      </Button>
    </span>
  );
}

/** B6: the node's snooze, until a date or until another node is done or out of scope; a container's holds over its subtree. */
export function SnoozeEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `snooze:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const dateForm = useFormDraft<string>(write.journey, key, "snooze-date");
  const nodeForm = useFormDraft<string>(write.journey, key, "snooze-node");
  const stored = view.journey.graph.state?.snoozes?.[key];
  const via = detail.derived.snoozed_via ?? undefined;
  const viaStored = via === undefined ? undefined : view.journey.graph.state?.snoozes?.[via];
  // With a container's snooze over it, `snoozed` is the node's own target while that holds, else the container's.
  const holds = via === undefined ? detail.derived.snoozed != null : JSON.stringify(detail.derived.snoozed) === JSON.stringify(stored);
  const snoozable = !isTerminal(detail.record.state);
  return (
    <div className="stack" data-testid="snooze">
      {stored === undefined ? null : (
        <span className="row">
          <span>
            Snoozed until <SnoozeUntil view={view} target={stored} />
            {holds ? "" : " (no longer holding)"}
          </span>
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "unsnooze", node: key }])}>
            Unsnooze
          </Button>
        </span>
      )}
      {via === undefined || viaStored === undefined ? null : <SnoozedThrough view={view} write={write} container={via} target={viaStored} />}
      {snoozable ? (
        <span className="row">
          <DraftForm
            write={write}
            draft={dateForm}
            form="snooze-date"
            start={view.key.today}
            label="Snooze until a date"
            input={dateInput("Snooze until")}
            mutations={(date) => [{ op: "snooze", node: key, until: { date } }]}
          />
          <DraftForm
            write={write}
            draft={nodeForm}
            form="snooze-node"
            start=""
            label="Snooze until a node"
            input={(value, change) => (
              <select aria-label="Snooze until node" value={value} onChange={(event) => { change(event.target.value); }}>
                <option value="">Choose</option>
                {(view.journey.graph.nodes ?? [])
                  .filter((node) => node.key !== key)
                  .map((node) => (
                    <option key={node.key} value={node.key}>
                      {node.title}
                    </option>
                  ))}
              </select>
            )}
            mutations={(node) => [{ op: "snooze", node: key, until: { node } }]}
          />
        </span>
      ) : null}
      <Rejected view={view} write={write} />
    </div>
  );
}

/** B5, Gating: force include with its reason, or lifting it. */
export function ForceInclude({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `force-include:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const reasonForm = useFormDraft<string>(write.journey, key, "force-include");
  const forced = detail.overrides?.force_include;
  return (
    <div className="stack" data-testid="force-include">
      {forced == null ? (
        detail.derived.relevance.value === "relevant" ? null : (
          <DraftForm
            write={write}
            draft={reasonForm}
            form="force-include"
            start=""
            label="Force include"
            input={textInput("Why include it")}
            mutations={(reason) => [{ op: "apply_override", node: key, override: { force_include: { reason } } }]}
          />
        )
      ) : (
        <span className="row">
          <span>Force included: {forced}</span>
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "remove_override", node: key, kind: "force_include" }])}>
            Lift
          </Button>
        </span>
      )}
      <Rejected view={view} write={write} />
    </div>
  );
}

/** B5: the node's weight in this journey; empty restores the kind's default. */
export function WeightEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `weight:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const weightForm = useFormDraft<string>(write.journey, key, "weight");
  const weight = detail.node.weight;
  return (
    <div className="stack" data-testid="weight">
      <span className="row">
        <span>Weight: {weight == null ? "the kind's default" : weight}</span>
        <DraftForm
          write={write}
          draft={weightForm}
          form="weight"
          start={weight == null ? "1" : String(weight)}
          label="Change the weight"
          input={(value, change) => (
            <Field type="number" min="0" step="any" aria-label="Weight" value={value} onChange={(event) => { change(event.target.value); }} />
          )}
          mutations={(value) => [{ op: "set_node_field", node: key, value: { weight: Number(value) } }]}
        />
        {weight == null ? null : (
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "set_node_field", node: key, value: { weight: null } }])}>
            Use the default
          </Button>
        )}
      </span>
      <Rejected view={view} write={write} />
    </div>
  );
}

/** B5, E2: set a kind's participation to explicit entities on this node, or restore inheritance. */
export function ParticipationEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `participation:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const form = useFormDraft<{ kind: string; chosen: string[] }>(write.journey, key, "participation");
  const kinds = ["k_owner", ...(view.journey.graph.participation_kinds ?? []).map((each) => each.key)];
  const own = Object.keys(nodeOf(view, key)?.participations ?? {});
  const entities = view.inputs.deployment.entities ?? [];
  const draft = form.draft;
  const save = async (kind: string, chosen: string[], seen: Seen) => {
    if (await write.run([{ op: "set_participation", node: key, kind, source: chosen }], seen)) {
      form.close();
    }
  };
  return (
    <div className="stack" data-testid="participation-editor">
      <span className="row">
        <select
          aria-label="Participation kind"
          value={draft?.value.kind ?? ""}
          onChange={(event) => { form.open({ kind: event.target.value, chosen: [] }, draft ?? write.seen); }}
        >
          <option value="">Set a participation</option>
          {kinds.map((each) => (
            <option key={each} value={each}>
              {each.replace(/^k_/, "")}
            </option>
          ))}
        </select>
        {own.map((each) => (
          <Button key={each} disabled={write.disabled} onClick={() => void write.run([{ op: "clear_participation", node: key, kind: each }])}>
            Inherit {each.replace(/^k_/, "")} again
          </Button>
        ))}
      </span>
      {draft === undefined || draft.value.kind === "" ? null : (
        <span className="row">
          {entities.map((entity) => (
            <label key={entity.key}>
              <input
                type="checkbox"
                checked={draft.value.chosen.includes(entity.key)}
                onChange={() => {
                  const { chosen } = draft.value;
                  form.change({ ...draft.value, chosen: chosen.includes(entity.key) ? chosen.filter((each) => each !== entity.key) : [...chosen, entity.key] });
                }}
              />{" "}
              {entityName(view, entity.key)}
            </label>
          ))}
          <Button primary disabled={write.disabled} onClick={() => void save(draft.value.kind, draft.value.chosen, draft)}>
            Save
          </Button>
          <Button onClick={() => { form.close(); }}>Cancel</Button>
        </span>
      )}
      <Rejected view={view} write={write} />
    </div>
  );
}
