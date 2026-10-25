// C9's actions over a selection, the same on every surface that holds one (the inspector's
// selection panel, the list's header): Status (Start, Mark done, Reopen, Skip), Assign owner, Snooze (a
// date, a node, Unsnooze, B6), always in that order. Each is one patch with one mutation, so
// one event, per node; a guard failure on any node rejects the whole patch, and the rejection
// names which node failed (A15), with D4's bypass. An action every member's state does not
// allow stays visible and disabled, its reason in the tooltip, so nothing is sent that is
// certain to be refused.
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useLocation, useNavigate } from "react-router";

import { UntilFields } from "../detail/editors.tsx";
import { titleOf, type Ready } from "../detail/model.ts";
import { snoozeStart, untilOf, withinScope, type SnoozeDraft } from "../detail/snooze.ts";
import { Menu } from "../screens/Menu.tsx";
import { Rejected } from "../detail/Rejected.tsx";
import { IdentityOptions } from "../detail/parts.tsx";
import { useFormDraft, useNodeWrite, type Seen } from "../detail/write.ts";
import { Button, Input } from "../ui/kit.tsx";
import { reasonFor, runBulk, type BulkAction, type Facts } from "./acts.ts";
import { ASSIGN, offerOf, selectionScope, SNOOZE, STATUS, type Form, type Offer, type SelectionChoice, type SelectionOfferId } from "./selectionOffers.ts";

/** What a form holds: its text, and for a snooze the same draft the inspector's snooze form keeps. */
interface Filled {
  form: Form;
  value: string;
  snooze: SnoozeDraft;
}

/** The action a filled form asks for, or none while it asks for nothing yet. */
function actionOf({ form, value, snooze }: Filled, today: string): BulkAction | undefined {
  const text = value.trim();
  switch (form) {
    case "skip":
      return text === "" ? undefined : { act: "skip", reason: text };
    case "assign":
      return text === "" ? undefined : { act: "assign", identity: text };
    case "snooze": {
      const until = untilOf(snooze, today);
      return until === undefined ? undefined : { act: "snooze", until };
    }
  }
}

/**
 * The choice the graph's context menu left in the navigation's state for these members: run
 * here, with the handlers the panel's own menus use, so a form opens in the panel and a plain
 * move is sent at once. It is taken off the history entry as it is run, so a reload does not
 * repeat it.
 */
function useSelectionChoice(scope: string, run: (id: SelectionOfferId) => void): void {
  const navigate = useNavigate();
  const location = useLocation();
  const chosen = (location.state as { selectionChoice?: SelectionChoice } | null)?.selectionChoice;
  const done = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (chosen === undefined || chosen.scope !== scope || done.current === location.key) {
      return;
    }
    done.current = location.key;
    void navigate(location, { replace: true, state: { ...(location.state as object), selectionChoice: undefined } });
    run(chosen.id);
  });
}

function FormInput({ view, open, within, onChange }: { view: Ready; open: Filled; within: ReadonlySet<string>; onChange: (next: Filled) => void }) {
  const group = useId();
  const { form, value } = open;
  if (form === "skip") {
    return (
      <label className="stack">
        <span className="label">Why skip them</span>
        <Input value={value} onChange={(event) => { onChange({ ...open, value: event.target.value }); }} />
      </label>
    );
  }
  if (form === "assign") {
    return (
      <select aria-label="Owner for them" value={value} onChange={(event) => { onChange({ ...open, value: event.target.value }); }}>
        <option value="">Choose</option>
        <IdentityOptions view={view} />
      </select>
    );
  }
  return <UntilFields view={view} group={group} exclude={within} value={open.snooze} onChange={(patch) => { onChange({ ...open, snooze: { ...open.snooze, ...patch } }); }} />;
}

/** The form an action opened: its field, Apply to every member, Cancel. */
function FormRow({ view, within, open, count, disabled, onChange, onApply, onCancel }: { view: Ready; within: ReadonlySet<string>; open: Filled; count: number; disabled: boolean; onChange: (next: Filled) => void; onApply: () => void; onCancel: () => void }) {
  return (
    <div className="row" data-testid="bulk-form" data-form={open.form}>
      <FormInput view={view} open={open} within={within} onChange={onChange} />
      <Button primary disabled={disabled || actionOf(open, view.derived.today) === undefined} onClick={onApply}>
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
  const scope = selectionScope(selected.map((facts) => facts.node.key));
  const draft = useFormDraft<Filled>(write.journey, "selection", "bulk-form");
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
      draft.open({ form: offer.form, value: "", snooze: snoozeStart(view, "") }, write.seen);
    }
  };
  useSelectionChoice(scope, (id) => { pick(offerOf(id)); });
  const group = (label: string, offers: Offer[]) => {
    const reasons = offers.map(why);
    const none = reasons.every((reason) => reason !== undefined);
    return (
      <Menu label={label} testId={`selection-${label.toLowerCase()}`} trigger={`${label} ▾`} tip={none ? reasons[0] : undefined}>
        {(close) =>
          offers.map((offer, at) => (
            <Button key={offer.words} role="menuitem" className="menu-item" disabled={write.disabled} held={write.disabled ? undefined : reasons[at]} onClick={() => { close(); pick(offer); }}>
              {offer.words}
            </Button>
          ))
        }
      </Menu>
    );
  };
  if (!write.writable) {
    return null;
  }
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
        <FormRow view={view} within={new Set(selected.flatMap((facts) => [...withinScope(view, facts.node.key)]))} open={open.value} count={selected.length} disabled={write.disabled} onChange={draft.change} onApply={() => { const action = actionOf(open.value, view.derived.today); if (action !== undefined) { void act(action, open); } }} onCancel={draft.close} />
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
