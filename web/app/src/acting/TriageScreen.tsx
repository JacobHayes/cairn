// C11: triage, a card flow over the acting frontier, one focus card at a time in rank order,
// for "act, or move on"; and its decisions-only mode, the decision walkthrough, which starting
// a journey opens (address.ts, `walkthroughPath`). It is NEXT, CARDS, with DECISIONS on for the
// walkthrough: the journey page (screens/JourneyFrame.tsx) holds its toolbar and the filters
// it opens (`TriageControls`). Triage always reads the current frontier
// (the engine's `next` projection over the tab's derivation, kept current, H6), so an answer
// that unblocks new nodes surfaces them in the same pass. Pass is client state for this pass
// only (pass.ts): kept per tab in session storage, never sent.
import { useCallback, useEffect, useMemo } from "react";

import { useProjected } from "../canvas/hooks.ts";
import { useDraft } from "../data/drafts.ts";
import { titleOf, type Ready } from "../detail/model.ts";
import { Button } from "../ui/kit.tsx";
import { ACTING_KINDS, triageQueryOf, type TriageSettings } from "./address.ts";
import { Check, Checks } from "./Controls.tsx";
import { DetailLink } from "./Parts.tsx";
import { begin, passedAll, passOn, passOrder, surfaced, type Pass } from "./pass.ts";
import { StalledPanel } from "./Stalled.tsx";
import { TriageCard } from "./TriageCard.tsx";
import { WaitingDecisions } from "./Waiting.tsx";
import { actingDecisions } from "./waiting.ts";
import "./acting.css";

/** How many cards after the focus the pass shows by title. */
const UP_NEXT_SHOWN = 5;

/** C11: mine and kinds: what the filter holds on NEXT, CARDS. */
export function TriageControls({ settings, onChange }: { settings: TriageSettings; onChange: (next: TriageSettings) => void }) {
  return (
    <div className="stack acting-controls" data-testid="triage-controls" data-mode={settings.decisions ? "decisions" : "all"}>
      <Check label="Only mine" checked={settings.mine} testId="only-mine" onChange={(mine) => { onChange({ ...settings, mine }); }} />
      {settings.decisions ? null : (
        <Checks legend="Kinds" options={ACTING_KINDS} chosen={settings.kinds} words={(kind) => kind} testId="kind" stacked onChange={(kinds) => { onChange({ ...settings, kinds }); }} />
      )}
    </div>
  );
}

/** What reached the acting frontier since the pass began: an answer's newly unblocked nodes. */
function Surfaced({ view, keys }: { view: Ready; keys: string[] }) {
  if (keys.length === 0) {
    return null;
  }
  return (
    <p className="callout row" data-testid="surfaced">
      <span>New to act on since this pass began:</span>
      {keys.map((key) => (
        <DetailLink key={key} view={view} node={key} />
      ))}
    </p>
  );
}

/**
 * No card to show: the filters hide what can be acted on; or, in the walkthrough, no decision
 * can be made by anyone, so what would unblock the next ones; or the journey is stalled (D5);
 * or nothing is left.
 */
function Empty({ view, settings, stalled }: { view: Ready; settings: TriageSettings; stalled: boolean }) {
  const filtered = settings.decisions ? actingDecisions(view).length > 0 : view.derived.acting_frontier.length > 0;
  if (filtered) {
    return (
      <p className="callout" data-testid="triage-empty" data-status="filtered">
        Nothing on the acting frontier matches these filters{settings.decisions ? ": the decisions open now are others'" : ""}.
      </p>
    );
  }
  if (settings.decisions) {
    return <WaitingDecisions view={view} />;
  }
  return stalled ? (
    <StalledPanel view={view} />
  ) : (
    <p className="callout" data-testid="triage-empty" data-status="none">
      Nothing is left to act on in this journey.
    </p>
  );
}

/** The journey's pass, begun over its acting frontier the first time triage opens in this tab. */
function usePass(view: Ready): [Pass, (pass: Pass) => void] {
  const [stored, setStored] = useDraft<Pass>(`triage-pass:${view.journey.header.id}`);
  const frontier = view.derived.acting_frontier;
  const pass = useMemo(() => stored ?? begin(frontier), [stored, frontier]);
  useEffect(() => {
    if (stored === undefined) {
      setStored(pass);
    }
  }, [stored, pass, setStored]);
  return [pass, setStored];
}

/** NEXT, CARDS: one focus card at a time, in the order of this pass. */
export function TriageBody({ view, settings, selected }: { view: Ready; settings: TriageSettings; selected: string | undefined }) {
  const [pass, setPass] = usePass(view);
  const request = useMemo(() => ({ projection: "next" as const, query: triageQueryOf(settings) }), [settings]);
  const { value: next, error } = useProjected(view, request);
  const wanted = settings.text.trim().toLowerCase();
  const rows = (next?.items ?? []).filter((row) => wanted === "" || titleOf(view, row.key).toLowerCase().includes(wanted));
  const order = passOrder(rows.map((row) => row.key), pass);
  const focus = rows.find((row) => row.key === order[0]);
  const onPass = useCallback(() => {
    if (focus !== undefined) {
      setPass(passOn(pass, focus.key));
    }
  }, [focus, pass, setPass]);
  const done = passedAll(order, pass);
  const passedCount = order.filter((key) => pass.passed.includes(key)).length;
  return (
    <section className="stack" aria-label={settings.decisions ? "Decision walkthrough" : "Triage"} data-testid="triage" data-order={order.join(" ")}>
      <span className="row acting-pass-controls">
        <Button onClick={() => { setPass(begin(view.derived.acting_frontier)); }}>Start a new pass</Button>
      </span>
      {error === undefined ? null : <p className="callout callout-bad">The frontier could not be read: {error}</p>}
      <Surfaced view={view} keys={surfaced(view.derived.acting_frontier, pass)} />
      {next === undefined ? <p className="muted small">Reading the frontier...</p> : null}
      {next !== undefined && rows.length === 0 ? <Empty view={view} settings={settings} stalled={next.stalled != null} /> : null}
      {done ? (
        <p className="callout" data-testid="pass-done">
          Every card has been passed once in this pass; they come round again in the order you passed them.
        </p>
      ) : null}
      {focus === undefined ? null : <TriageCard key={focus.key} view={view} row={focus} position={Math.min(passedCount + 1, order.length)} total={order.length} onPass={onPass} inspected={focus.key === selected} />}
      {order.length > 1 ? (
        <section className="stack" aria-label="Up next" data-testid="up-next">
          <span className="muted small">Up next in this pass:</span>
          <ol className="detail-list">
            {order.slice(1, 1 + UP_NEXT_SHOWN).map((key) => (
              <li key={key} data-node={key}>
                <DetailLink view={view} node={key} />
              </li>
            ))}
          </ol>
        </section>
      ) : null}
    </section>
  );
}
