// C9's bulk actions over the list's selection: transition (start, done), skip with its reason,
// assign an owner, snooze until a date or a node, unsnooze (B6). Each is one patch with one
// mutation, so one event, per node; a guard failure on any node rejects the whole patch, and
// the rejection names which node failed (A15), with D4's bypass. A node whose kind or state
// cannot take the action stops it before anything is sent, named.
import { useState } from "react";

import { titleOf, type Ready } from "../detail/model.ts";
import { Rejected } from "../detail/Rejected.tsx";
import { entityName } from "../detail/sections.tsx";
import { useFormDraft, useNodeWrite, type Seen } from "../detail/write.ts";
import { Button, Field } from "../ui/kit.tsx";
import { runBulk, type BulkAction, type Facts } from "./acts.ts";

type Form = "skip" | "assign" | "snooze-date" | "snooze-node";

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
    <select className="select" aria-label={form === "assign" ? "Owner for them" : "Snooze them until node"} value={value} onChange={(event) => { onChange(event.target.value); }}>
      <option value="">Choose</option>
      {options.map((option) => (
        <option key={option.key} value={option.key}>
          {option.text}
        </option>
      ))}
    </select>
  );
}

/** C9: the bulk actions over `selected`, of which `hidden` are not on the page shown. */
export function BulkBar({ view, selected, hidden, onLanded }: { view: Ready; selected: Facts[]; hidden: number; onLanded: () => void }) {
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
  return (
    <div className="panel stack bulk-bar" data-testid="bulk-bar" data-count={selected.length}>
      <div className="row">
        <strong>{selected.length} selected</strong>
        {hidden === 0 ? null : <span className="muted" data-testid="selected-elsewhere">({hidden} on other pages)</span>}
        <Button disabled={write.disabled} onClick={() => void act({ act: "start" })}>Start</Button>
        <Button disabled={write.disabled} onClick={() => void act({ act: "done" })}>Done</Button>
        <Button disabled={write.disabled} onClick={() => void act({ act: "unsnooze" })}>Unsnooze</Button>
        {(Object.keys(FORM_WORDS) as Form[]).map((form) => (
          <Button key={form} disabled={write.disabled} onClick={() => { draft.open({ form, value: form === "snooze-date" ? view.key.today : "" }, write.seen); }}>
            {FORM_WORDS[form]}
          </Button>
        ))}
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
