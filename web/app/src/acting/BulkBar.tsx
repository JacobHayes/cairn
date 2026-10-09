// C9's bulk actions over the list's selection: transition (start, done), skip with its reason,
// assign an owner, snooze until a date or a node, unsnooze (B6). Each is one patch with one
// mutation, so one event, per node; a guard failure on any node rejects the whole patch, and
// the rejection names which node failed (A15), with D4's bypass. A node whose kind or state
// cannot take the action stops it before anything is sent, named.
import { useState, type ReactNode } from "react";

import { titleOf, type Ready } from "../detail/model.ts";
import { Menu } from "../screens/Menu.tsx";
import { Rejected } from "../detail/Rejected.tsx";
import { entityName } from "../detail/sections.tsx";
import { useFormDraft, useNodeWrite, type Seen } from "../detail/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { canTake, runBulk, type BulkAction, type Facts } from "./acts.ts";

type Form = "skip" | "assign" | "snooze-date" | "snooze-node";

/** The actions the bar names: the first few sit inline, the rest under More. */
const INLINE_MAX = 3;

const FORM_WORDS: Record<Form, string> = {
  skip: "Skip...",
  assign: "Assign owner...",
  "snooze-date": "Snooze until a date...",
  "snooze-node": "Snooze until a node...",
};

/** The action a filled form asks for. */
function actionOf(form: Form, value: string): BulkAction {
  switch (form) {
    case "skip":
      return { act: "skip", reason: value };
    case "assign":
      return { act: "assign", entity: value };
    case "snooze-date":
      return { act: "snooze", until: { date: value } };
    case "snooze-node":
      return { act: "snooze", until: { node: value } };
  }
}

/** One action the bar can offer: what it says, and the action it asks for (a form's, before it is filled). */
interface Offer {
  words: string;
  asks: BulkAction;
  /** A form to open, when the action needs more than a click. */
  form?: Form;
}

const OFFERS: Offer[] = [
  { words: "Start", asks: { act: "start" } },
  { words: "Done", asks: { act: "done" } },
  { words: "Unsnooze", asks: { act: "unsnooze" } },
  ...(Object.keys(FORM_WORDS) as Form[]).map((form): Offer => ({ words: FORM_WORDS[form], asks: actionOf(form, ""), form })),
];

function FormInput({ view, form, value, onChange }: { view: Ready; form: Form; value: string; onChange: (value: string) => void }) {
  if (form === "skip") {
    return <Field aria-label="Why skip them" placeholder="Why" value={value} onChange={(event) => { onChange(event.target.value); }} />;
  }
  if (form === "snooze-date") {
    return <Field type="date" aria-label="Snooze them until" value={value} onChange={(event) => { onChange(event.target.value); }} />;
  }
  const options =
    form === "assign"
      ? (view.inputs.deployment.entities ?? []).map((entity) => ({ key: entity.key, text: entityName(view, entity.key) }))
      : (view.journey.graph.nodes ?? []).map((node) => ({ key: node.key, text: node.title }));
  return (
    <select aria-label={form === "assign" ? "Owner for them" : "Snooze them until node"} value={value} onChange={(event) => { onChange(event.target.value); }}>
      <option value="">Choose</option>
      {options.map((option) => (
        <option key={option.key} value={option.key}>
          {option.text}
        </option>
      ))}
    </select>
  );
}

/**
 * C9: the bulk actions over `selected`, of which `hidden` are not in the list shown; `onClear` drops the
 * selection. While a journey's structure is edited, `structure` stands in for the actions: the bar offers
 * what the edit mode does to a selection (Remove), not what a journey's work does.
 */
export function BulkBar({ view, selected, hidden, onLanded, onClear, structure }: { view: Ready; selected: Facts[]; hidden: number; onLanded: () => void; onClear?: () => void; structure?: ReactNode }) {
  const write = useNodeWrite(view, "bulk");
  const draft = useFormDraft<{ form: Form; value: string }>(write.journey, "selection", "bulk");
  const [unable, setUnable] = useState<string[]>([]);
  const act = async (action: BulkAction, seen: Seen = write.seen) => {
    const sent = await runBulk(action, selected, (mutations) => write.run(mutations, seen));
    setUnable(sent.outcome === "unable" ? sent.unable : []);
    if (sent.outcome === "sent" && sent.landed) {
      draft.close();
      onLanded();
    }
  };
  const open = draft.draft;
  // Only what some selected node can take is offered: the others would only be refused.
  const offered = structure === undefined ? OFFERS.filter((offer) => selected.some((facts) => canTake(offer.asks, facts))) : [];
  const pick = (offer: Offer) => {
    if (offer.form === undefined) {
      void act(offer.asks);
    } else {
      draft.open({ form: offer.form, value: offer.form === "snooze-date" ? view.key.today : "" }, write.seen);
    }
  };
  return (
    <div className="panel stack bulk-bar" data-testid="bulk-bar" data-count={selected.length}>
      <div className="row">
        <strong>{selected.length} selected</strong>
        {hidden === 0 ? null : <span className="muted small" data-testid="selected-elsewhere">({hidden} not shown)</span>}
        {structure}
        {offered.slice(0, INLINE_MAX).map((offer) => (
          <Button key={offer.words} disabled={write.disabled} onClick={() => { pick(offer); }}>{offer.words}</Button>
        ))}
        {offered.length <= INLINE_MAX ? null : (
          <Menu label="More actions" testId="bulk-more" trigger="More ▾">
            {(close) =>
              offered.slice(INLINE_MAX).map((offer) => (
                <button key={offer.words} type="button" role="menuitem" className="menu-item" disabled={write.disabled} onClick={() => { close(); pick(offer); }}>
                  {offer.words}
                </button>
              ))
            }
          </Menu>
        )}
        {onClear === undefined ? null : <Button ghost onClick={onClear}>Clear selection</Button>}
      </div>
      {open === undefined ? null : (
        <div className="row" data-testid="bulk-form" data-form={open.value.form}>
          <FormInput view={view} form={open.value.form} value={open.value.value} onChange={(value) => { draft.change({ ...open.value, value }); }} />
          <Button primary disabled={write.disabled || open.value.value.trim() === ""} onClick={() => void act(actionOf(open.value.form, open.value.value.trim()), open)}>
            Apply to {selected.length}
          </Button>
          <Button onClick={() => { draft.close(); }}>Cancel</Button>
        </div>
      )}
      {unable.length === 0 ? null : (
        <p className="callout" role="alert" data-testid="bulk-unable">
          Nothing was sent: {unable.map((key) => titleOf(view, key)).join(", ")} cannot take that action in {unable.length === 1 ? "its" : "their"} current state.
        </p>
      )}
      <Rejected view={view} write={write} />
    </div>
  );
}
