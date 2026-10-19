// C9's actions over a selection, the same on every surface that holds one (the inspector's
// selection panel, the list's header): Status (Start, Mark done, Skip), Assign owner, Snooze (a
// date, a node, Unsnooze, B6), always in that order. Each is one patch with one mutation, so
// one event, per node; a guard failure on any node rejects the whole patch, and the rejection
// names which node failed (A15), with D4's bypass. An action every member's state does not
// allow stays visible and disabled, its reason in the tooltip, so nothing is sent that is
// certain to be refused.
import { useEffect, useRef, useState, type ReactNode } from "react";

import { UntilNode } from "../detail/editors.tsx";
import { titleOf, type Ready } from "../detail/model.ts";
import { ACT_LABELS } from "../detail/offers.ts";
import { withinScope } from "../detail/snooze.ts";
import { Menu } from "../screens/Menu.tsx";
import { Rejected } from "../detail/Rejected.tsx";
import { EntityOptions } from "../detail/parts.tsx";
import { useFormDraft, useNodeWrite, type Seen } from "../detail/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { reasonFor, runBulk, type BulkAction, type Facts } from "./acts.ts";

type Form = "skip" | "assign" | "snooze-date" | "snooze-node";

const FORM_WORDS: Record<Form, string> = {
  skip: "Skip…",
  assign: "Assign owner…",
  "snooze-date": "Snooze until a date…",
  "snooze-node": "Snooze until a node…",
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

/** One entry of a menu: what it says, the action it asks for (a form's, before it is filled), and the form it opens. */
interface Offer {
  words: string;
  asks: BulkAction;
  form?: Form;
}

const form = (kind: Form): Offer => ({ words: FORM_WORDS[kind], asks: actionOf(kind, ""), form: kind });

const STATUS: Offer[] = [{ words: ACT_LABELS.start, asks: { act: "start" } }, { words: ACT_LABELS.done, asks: { act: "done" } }, form("skip")];
const ASSIGN = form("assign");
const SNOOZE: Offer[] = [form("snooze-date"), form("snooze-node"), { words: ACT_LABELS.unsnooze, asks: { act: "unsnooze" } }];

function FormInput({ view, form, within, value, onChange }: { view: Ready; form: Form; within: ReadonlySet<string>; value: string; onChange: (value: string) => void }) {
  if (form === "skip") {
    return <Field aria-label="Why skip them" placeholder="Why" value={value} onChange={(event) => { onChange(event.target.value); }} />;
  }
  if (form === "snooze-date") {
    return <Field type="date" aria-label="Snooze them until" value={value} onChange={(event) => { onChange(event.target.value); }} />;
  }
  if (form === "assign") {
    return (
      <select aria-label="Owner for them" value={value} onChange={(event) => { onChange(event.target.value); }}>
        <option value="">Choose</option>
        <EntityOptions view={view} />
      </select>
    );
  }
  return <UntilNode view={view} label="Snooze them until node" exclude={within} value={value} onChange={onChange} />;
}

/** The form an action opened: its field, Apply to every member, Cancel. */
function FormRow({ view, within, open, count, disabled, onChange, onApply, onCancel }: { view: Ready; within: ReadonlySet<string>; open: { form: Form; value: string }; count: number; disabled: boolean; onChange: (value: string) => void; onApply: () => void; onCancel: () => void }) {
  return (
    <div className="row" data-testid="bulk-form" data-form={open.form}>
      <FormInput view={view} form={open.form} within={within} value={open.value} onChange={onChange} />
      <Button primary disabled={disabled || open.value.trim() === ""} onClick={onApply}>
        Apply to {count}
      </Button>
      <Button onClick={onCancel}>Cancel</Button>
    </div>
  );
}

/**
 * C9: the actions over `selected`; `onLanded` runs once one lands. While a journey's structure
 * is edited, `structure` stands in for them: what the edit mode does to a selection (Remove).
 */
export function SelectionActions({ view, selected, onLanded, structure }: { view: Ready; selected: Facts[]; onLanded?: () => void; structure?: ReactNode }) {
  const write = useNodeWrite(view, "bulk");
  // A form, and what a refusal said, belong to the members they were for, and go with them or with the panel.
  const scope = selected.map((facts) => facts.node.key).sort().join(",");
  const draft = useFormDraft<{ form: Form; value: string }>(write.journey, "selection", "bulk");
  const [refused, setRefused] = useState<{ scope: string; keys: string[] }>({ scope, keys: [] });
  const unable = refused.scope === scope ? refused.keys : [];
  const closing = useRef(draft.close);
  closing.current = draft.close;
  useEffect(() => () => { closing.current(); }, [scope]);
  const act = async (action: BulkAction, seen: Seen = write.seen) => {
    const sent = await runBulk(action, selected, (mutations) => write.run(mutations, seen));
    setRefused({ scope, keys: sent.outcome === "unable" ? sent.unable : [] });
    if (sent.outcome === "sent" && sent.landed) {
      draft.close();
      onLanded?.();
    }
  };
  const open = draft.draft;
  const why = (offer: Offer) => reasonFor(offer.asks, selected, (key) => titleOf(view, key));
  const pick = (offer: Offer) => {
    if (offer.form === undefined) {
      void act(offer.asks);
    } else {
      draft.open({ form: offer.form, value: offer.form === "snooze-date" ? view.key.today : "" }, write.seen);
    }
  };
  const group = (label: string, offers: Offer[]) => {
    const reasons = offers.map(why);
    const none = reasons.every((reason) => reason !== undefined);
    return (
      <Menu label={label} testId={`selection-${label.toLowerCase()}`} trigger={`${label} ▾`} title={none ? (reasons[0] ?? label) : label}>
        {(close) =>
          offers.map((offer, at) => (
            <button key={offer.words} type="button" role="menuitem" className="menu-item" disabled={write.disabled || reasons[at] !== undefined} title={reasons[at]} onClick={() => { close(); pick(offer); }}>
              {offer.words}
            </button>
          ))
        }
      </Menu>
    );
  };
  return (
    <div className="stack" data-testid="selection-actions" data-count={selected.length}>
      <div className="row">
        {structure ?? (
          <>
            {group("Status", STATUS)}
            <Button disabled={write.disabled} onClick={() => { pick(ASSIGN); }}>{ASSIGN.words}</Button>
            {group("Snooze", SNOOZE)}
          </>
        )}
      </div>
      {open === undefined ? null : (
        <FormRow view={view} within={new Set(selected.flatMap((facts) => [...withinScope(view, facts.node.key)]))} open={open.value} count={selected.length} disabled={write.disabled} onChange={(value) => { draft.change({ ...open.value, value }); }} onApply={() => void act(actionOf(open.value.form, open.value.value.trim()), open)} onCancel={draft.close} />
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
