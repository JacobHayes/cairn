// The node's journey state a section edits in place (B5, B6, E3, F5): its pin, routed
// through the decision that feeds it when one does; the snooze form (what to set aside, until
// when) the actions open; a force include lifted; its weight; and its participations. Each is
// one patch whose rejection shows here, a pin's with the moves that resolve a contradictory chain.
import { OWNER_KIND } from "@cairn/client";
import { useId, type ReactNode } from "react";

import { snoozeEndWords } from "../status/snooze.ts";
import { dateWords } from "../status/when.ts";
import { Button, Input } from "../ui/kit.tsx";
import { Saving, type Sending } from "../ui/Receipt.tsx";
import { WhenInput } from "../ui/WhenInput.tsx";
import { type Mutation, type NodeDetail, nodeOf, type Ready, titleOf, transition } from "./model.ts";
import { NodeLink } from "./parts.tsx";
import { Rejected } from "./Rejected.tsx";
import { identityName } from "./sections.tsx";
import { setAsideOptions, UNTIL_OPTIONS, untilOf, withinScope, type SnoozeDraft } from "./snooze.ts";
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
  if (!write.writable) {
    return null;
  }
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

const dateInput = (label: string, today: string, zone: string) => (value: string, change: (value: string) => void) => <WhenInput label={label} today={today} value={value} zone={zone} onChange={change} />;

/**
 * F2, E3: the node's pin. A milestone a date decision feeds is pinned only through that
 * decision: setting its date answers the decision, and clearing it reopens the decision. Unpin
 * is offered while the form is closed, so a refusal's own Unpin move is the only one beside it.
 */
export function PinEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `pin:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const pinForm = useFormDraft<string>(write.journey, key, "pin");
  const fedBy = detail.fedBy;
  const fedAnswer = fedBy === undefined ? undefined : view.derived.nodes[fedBy.key]?.answer;
  const current = fedBy === undefined ? detail.pin : fedAnswer != null && "date" in fedAnswer ? fedAnswer.date : undefined;
  const set = (date: string): Mutation[] =>
    fedBy === undefined ? [{ op: "set_pin", node: key, date }] : [{ op: "answer", decision: fedBy.key, value: { date } }];
  const clear: Mutation = fedBy === undefined ? { op: "clear_pin", node: key } : transition(fedBy.key, "reopen");
  return (
    <div className="stack" data-testid="pin">
      {current === undefined || current === detail.derived.dates.due?.date ? null : (
        <span>
          Pinned to <strong data-testid="pin-date">{dateWords(current, view.derived.today)}</strong>
        </span>
      )}
      {fedBy === undefined || !write.writable ? null : (
        <span className="muted small" data-testid="pin-through">
          The pin is set by answering <NodeLink view={view} node={fedBy.key} />: editing it here answers that decision
        </span>
      )}
      <span className="row">
        <DraftForm
          write={write}
          draft={pinForm}
          form="pin"
          start={current ?? view.key.today}
          label={current === undefined ? "Pin a date" : "Change the pin"}
          input={dateInput("Pin date", view.derived.today, view.inputs.timezone)}
          mutations={set}
        />
        {current === undefined || !write.writable || pinForm.draft !== undefined ? null : (
          <Button disabled={write.disabled} onClick={() => void write.run([clear])}>
            {fedBy === undefined ? "Unpin" : "Unpin (reopens the decision)"}
          </Button>
        )}
      </span>
      <Rejected view={view} write={write} onResolved={pinForm.close} />
    </div>
  );
}

/** B6: the node, or a container above it with its open count: what a snooze sets aside. */
function SetAside({ options, group, aside, onChange }: { options: ReturnType<typeof setAsideOptions>; group: string; aside: string; onChange: (node: string) => void }) {
  return (
    <fieldset className="stack">
      <legend className="small muted">Set aside</legend>
      <div className="opts">
        {options.map((option) => (
          <label key={option.node} className="opt">
            <input type="radio" name={`${group}-aside`} checked={aside === option.node} onChange={() => { onChange(option.node); }} />
            <span>{option.open === undefined ? option.title : `All of ${option.title}`}</span>
            {option.open === undefined ? null : <span className="eff">{option.open} open</span>}
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** B6: the node a snooze waits on: any but `exclude`, the work being set aside, which the engine would refuse. */
export function UntilNode({ view, label, exclude, value, onChange }: { view: Ready; label: string; exclude: ReadonlySet<string>; value: string; onChange: (node: string) => void }) {
  return (
    <select aria-label={label} value={value} onChange={(event) => { onChange(event.target.value); }}>
      <option value="">Choose</option>
      {(view.journey.graph.nodes ?? [])
        .filter((node) => !exclude.has(node.key))
        .map((node) => (
          <option key={node.key} value={node.key}>
            {node.title}
          </option>
        ))}
    </select>
  );
}

/** B6: until when: a quick date, a date, or another node being done. */
export function UntilFields({ view, group, exclude, value, onChange }: { view: Ready; group: string; exclude: ReadonlySet<string>; value: SnoozeDraft; onChange: (patch: Partial<SnoozeDraft>) => void }) {
  const { until, date, target } = value;
  return (
    <>
      <fieldset className="stack">
        <legend className="small muted">Until</legend>
        <div className="opts">
          {UNTIL_OPTIONS.map((option) => (
            <label key={option.value} className="opt">
              <input type="radio" name={`${group}-until`} checked={until === option.value} onChange={() => { onChange({ until: option.value }); }} />
              <span>{option.label}</span>
            </label>
          ))}
        </div>
      </fieldset>
      {until === "date" ? <WhenInput label="Snooze until" today={view.derived.today} value={date} zone={view.inputs.timezone} onChange={(next) => { onChange({ date: next }); }} /> : null}
      {until === "node" ? <UntilNode view={view} label="Snooze until node" exclude={exclude} value={target} onChange={(next) => { onChange({ target: next }); }} /> : null}
    </>
  );
}

/** B6: what a snooze says it does, as a sentence's end: "Kickoff until Oct 20". */
export function snoozeWords(view: Ready, draft: SnoozeDraft): string {
  const until = untilOf(draft, view.derived.today);
  const when = until === undefined ? "a time not chosen" : snoozeEndWords(view, until);
  return `${titleOf(view, draft.aside)} until ${when}`;
}

/**
 * B6: the snooze form (shown in place of the buttons while it is open): what to set aside
 * (the node, or a container above it with its open count, nearest first) and until when.
 * Setting a container aside holds everything beneath it; the engine refuses a wait on the
 * set-aside work itself. It holds no state and sends nothing: its owner keeps the draft and
 * submits the write (against the revisions the form was opened at), and `footer` is where the
 * owner shows that write's rejection, inside the dialog.
 */
export function SnoozePanel({
  view,
  detail,
  value,
  sending,
  disabled,
  onChange,
  onSnooze,
  onClose,
  footer,
}: {
  view: Ready;
  detail: NodeDetail;
  value: SnoozeDraft;
  sending: Sending;
  disabled: boolean;
  onChange: (next: SnoozeDraft) => void;
  onSnooze: (mutation: Mutation) => void;
  onClose: () => void;
  footer: ReactNode;
}) {
  const group = useId();
  const options = setAsideOptions(view, detail);
  const { aside } = value;
  const change = (patch: Partial<SnoozeDraft>) => {
    onChange({ ...value, ...patch });
  };
  const chosen = untilOf(value, view.derived.today);
  return (
    <div className="stack popover-panel" role="dialog" aria-label="Snooze" data-testid="snooze" data-popover="">
      {options.length === 1 ? null : <SetAside options={options} group={group} aside={aside} onChange={(node) => { change({ aside: node }); }} />}
      <UntilFields view={view} group={group} exclude={withinScope(view, aside)} value={value} onChange={change} />
      <span className="row">
        <Button
          primary
          disabled={disabled || chosen === undefined}
          onClick={() => {
            if (chosen !== undefined) {
              onSnooze({ op: "snooze", node: aside, until: chosen });
            }
          }}
        >
          Snooze
        </Button>
        <Button onClick={onClose}>{sending ? "Close" : "Cancel"}</Button>
      </span>
      <Saving sending={sending} />
      {footer}
    </div>
  );
}

/** B5, Gating: a force include's reason, and lifting it. Making one is `⋯ › Include anyway`. */
export function ForceInclude({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `force-include:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const forced = detail.overrides?.force_include;
  if (forced == null) {
    return null;
  }
  return (
    <div className="stack" data-testid="force-include">
      <span className="row">
        <span>Included anyway: {forced}</span>
        {!write.writable ? null : (
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "remove_override", node: key, kind: "force_include" }])}>
            Lift
          </Button>
        )}
      </span>
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
            <Input type="number" min="0" step="any" aria-label="Weight" value={value} onChange={(event) => { change(event.target.value); }} />
          )}
          mutations={(value) => [{ op: "set_node_field", node: key, value: { weight: Number(value) } }]}
        />
        {weight == null || !write.writable ? null : (
          <Button disabled={write.disabled} onClick={() => void write.run([{ op: "set_node_field", node: key, value: { weight: null } }])}>
            Use the default
          </Button>
        )}
      </span>
      <Rejected view={view} write={write} />
    </div>
  );
}

/** B5, E2: set a kind's participation to explicit identities on this node, or restore inheritance. */
export function ParticipationEditor({ view, detail }: { view: Ready; detail: NodeDetail }) {
  const write = useNodeWrite(view, `participation:${detail.node.key}`, detail.node.key);
  const key = detail.node.key;
  const form = useFormDraft<{ kind: string; chosen: string[] }>(write.journey, key, "participation");
  const kinds: string[] = [OWNER_KIND.key, ...(view.journey.graph.participation_kinds ?? []).map((each) => each.key)];
  const own = Object.keys(nodeOf(view, key)?.participations ?? {});
  const identities = view.inputs.deployment.identities ?? [];
  const draft = form.draft;
  const save = async (kind: string, chosen: string[], seen: Seen) => {
    if (await write.run([{ op: "set_participation", node: key, kind, source: chosen }], seen)) {
      form.close();
    }
  };
  if (!write.writable) {
    return null;
  }
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
          {identities.map((identity) => (
            <label key={identity.key}>
              <input
                type="checkbox"
                checked={draft.value.chosen.includes(identity.key)}
                onChange={() => {
                  const { chosen } = draft.value;
                  form.change({ ...draft.value, chosen: chosen.includes(identity.key) ? chosen.filter((each) => each !== identity.key) : [...chosen, identity.key] });
                }}
              />{" "}
              {identityName(view, identity.key)}
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
