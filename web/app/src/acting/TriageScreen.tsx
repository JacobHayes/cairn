// C11: triage, a queue of cards over the acting frontier, one at a time in rank order, for "act,
// or move on"; and its decisions-only mode, the decision walkthrough, which starting a journey
// opens (address.ts, `walkthroughPath`). It is NEXT, CARDS, with DECISIONS on for the
// walkthrough: the journey page (screens/JourneyFrame.tsx) holds its toolbar and the filters it
// opens (`TriageControls`). A card is the inspector's own component with Pass on the frame's
// edge, apart from the node's actions; the inspector column holds the pass rail until a linked
// node is opened. Triage always reads the current frontier (the engine's `next` projection over
// the tab's derivation, kept current, H6), so an answer that unblocks new nodes surfaces them in
// the same pass. Pass is client state for this pass only (pass.ts): kept per tab in session
// storage, never sent. What a write made from the pass unlocked (the patch's consequences, never
// a diff of the frontier) comes next, each card labeled with the card that unlocked it.
import { useCallback, useEffect, useMemo, useRef } from "react";
import { useLocation, useNavigate } from "react-router";

import { useProjected } from "../canvas/hooks.ts";
import { useDraft } from "../data/drafts.ts";
import { useSaves, useSession } from "../data/react.ts";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import { titleOf, type Ready } from "../detail/model.ts";
import { screenPath } from "../detail/parts.tsx";
import { COLUMN, Inspector, useMedia } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { typing } from "../ui/typing.ts";
import { ACTING_KINDS, NEXT_FILTER_FLAGS, triagePath, triageQueryOf, type TriageSettings } from "./address.ts";
import { Check, Checks, FlagChecks } from "./Controls.tsx";
import { DetailLink } from "./Parts.tsx";
import { acted, begin, passedAll, passOn, passOrder, roundAgain, surfaced, unlockedBy, type Pass } from "./pass.ts";
import { PassRail } from "./PassRail.tsx";
import { withFlags } from "./rows.ts";
import { StalledPanel } from "./Stalled.tsx";
import { WaitingDecisions } from "./Waiting.tsx";
import type { NodeRow } from "./why.ts";
import { actingDecisions } from "./waiting.ts";
import "./acting.css";

/** C11: mine, kinds and flags: what the filter holds on NEXT, CARDS. */
export function TriageControls({ settings, onChange }: { settings: TriageSettings; onChange: (next: TriageSettings) => void }) {
  return (
    <div className="stack acting-controls" data-testid="triage-controls" data-mode={settings.decisions ? "decisions" : "all"}>
      <Check label="Only mine" checked={settings.mine} testId="only-mine" onChange={(mine) => { onChange({ ...settings, mine }); }} />
      {settings.decisions ? null : (
        <Checks legend="Kinds" options={ACTING_KINDS} chosen={settings.kinds} words={(kind) => kind} testId="kind" stacked onChange={(kinds) => { onChange({ ...settings, kinds }); }} />
      )}
      <FlagChecks options={NEXT_FILTER_FLAGS} chosen={settings.flags} onChange={(flags) => { onChange({ ...settings, flags }); }} />
    </div>
  );
}

/** What reached the acting frontier since the pass began, other than what the pass's own actions unlocked and its filters still show (the card's band and the rail name those). */
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
function Empty({ view, settings, stalled, onContinue }: { view: Ready; settings: TriageSettings; stalled: boolean; onContinue: () => void }) {
  const filtered = settings.decisions ? actingDecisions(view).length > 0 : view.derived.acting_frontier.length > 0;
  if (filtered) {
    return (
      <p className="callout" data-testid="triage-empty" data-status="filtered">
        Nothing on the acting frontier matches these filters{settings.decisions ? ": the decisions open now are others'" : ""}.
      </p>
    );
  }
  if (settings.decisions) {
    return <WaitingDecisions view={view} onContinue={onContinue} />;
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
  // A pass kept by an earlier version of the app has no unlocks or acted-on nodes.
  const [stored, setStored] = useDraft<Omit<Pass, "unlocked" | "acted"> & Partial<Pick<Pass, "unlocked" | "acted">>>(`triage-pass:${view.journey.header.id}`);
  const frontier = view.derived.acting_frontier;
  const pass = useMemo(() => (stored === undefined ? begin(frontier) : { ...stored, unlocked: stored.unlocked ?? [], acted: stored.acted ?? [] }), [stored, frontier]);
  useEffect(() => {
    if (stored === undefined) {
      setStored(pass);
    }
  }, [stored, pass, setStored]);
  return [pass, setStored];
}

/**
 * Hears the writes sent while the pass is on screen, the card's and those made in a node opened
 * from it: each one's unlocks come next. Writes sent before the pass opened (on another page,
 * even if they land after) and writes in another tab are never heard.
 */
function useUnlocks(journey: string, pass: Pass, setPass: (pass: Pass) => void): void {
  const saves = useSaves();
  const { activity } = useSession();
  const opened = useRef(activity.began);
  const heard = useRef(0);
  useEffect(() => {
    const fresh = saves.filter((save) => save.id > heard.current).reverse();
    if (fresh.length === 0) {
      return;
    }
    heard.current = Math.max(...fresh.map((save) => save.id));
    const next = fresh.reduce((now, { unlocks }) => (unlocks?.journey === journey && unlocks.began > opened.current ? acted(now, unlocks.by, unlocks.nodes) : now), pass);
    if (next !== pass) {
      setPass(next);
    }
  }, [saves, journey, pass, setPass]);
}

/** The card's keyboard: P passes, unless a field has the keys, a modifier is held, or the key sheet is open. */
function usePassKey(onPass: (() => void) | undefined): void {
  useEffect(() => {
    if (onPass === undefined) {
      return undefined;
    }
    const heard = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "p" && !event.ctrlKey && !event.metaKey && !event.altKey && !typing(event.target) && document.querySelector('[data-testid="key-sheet"]') === null) {
        event.preventDefault();
        onPass();
      }
    };
    document.addEventListener("keydown", heard);
    return () => {
      document.removeEventListener("keydown", heard);
    };
  }, [onPass]);
}

/** The card: the focus node's inspector panel, with its place in the pass and Pass on the frame's top edge. */
function PassCard({ view, card, place, by, onPass }: { view: Ready; card: NodeRow; place: string; by: string | undefined; onPass: () => void }) {
  return (
    <div className="pass-frame stack" data-testid="triage-card" data-node={card.key} data-kind={card.kind}>
      <span className="pass-edge row">
        <span className="row">
          <span className="muted small" data-testid="card-position">
            {place}
          </span>
          {by === undefined ? null : (
            <span className="small unlocked-by" data-testid="unlocked-by">
              <span className="label">↳ Unlocked by</span> <DetailLink view={view} node={by} />
            </span>
          )}
        </span>
        <Button onClick={onPass} data-testid="pass" title="Pass: later in this pass; nothing is saved">
          Pass <kbd>P</kbd>
        </Button>
      </span>
      <NodeDetailPanel key={card.key} view={view} nodeKey={card.key} folded />
    </div>
  );
}

/** NEXT, CARDS: one card at a time, in the order of this pass. */
export function TriageBody({ view, settings, selected }: { view: Ready; settings: TriageSettings; selected: string | undefined }) {
  const navigate = useNavigate();
  const { pathname, search } = useLocation();
  const wide = useMedia(COLUMN);
  const journey = view.journey.header.id;
  const [pass, setPass] = usePass(view);
  useUnlocks(journey, pass, setPass);
  const request = useMemo(() => ({ projection: "next" as const, query: triageQueryOf(settings) }), [settings]);
  const { value: next, error } = useProjected(view, request);
  const wanted = settings.text.trim().toLowerCase();
  const rows = withFlags(next?.items ?? [], settings.flags).filter((row) => wanted === "" || titleOf(view, row.key).toLowerCase().includes(wanted));
  const order = passOrder(rows.map((row) => row.key), pass);
  const focus = rows.find((row) => row.key === order[0]);
  const done = passedAll(order, pass);
  const card = done ? undefined : focus;
  const onPass = useCallback(() => {
    if (card !== undefined) {
      setPass(passOn(pass, card.key));
    }
  }, [card, pass, setPass]);
  usePassKey(card === undefined ? undefined : onPass);
  // The card is the focus node's inspector, so opening that node leaves one editor: the address drops it.
  useEffect(() => {
    if (selected !== undefined && selected === card?.key) {
      void navigate(`${screenPath(pathname)}${search}`, { replace: true });
    }
  }, [selected, card?.key, navigate, pathname, search]);
  const newPass = () => { setPass(begin(view.derived.acting_frontier)); };
  const passedCount = order.filter((key) => pass.passed.includes(key)).length;
  const rail = <PassRail view={view} order={order} pass={pass} onNewPass={newPass} />;
  const opened = selected !== undefined && selected !== card?.key;
  const open = settings.decisions ? actingDecisions(view).length : 0;
  return (
    <section className="stack pass" aria-label={settings.decisions ? "Decision walkthrough" : "Triage"} data-testid="triage" data-order={order.join(" ")}>
      {settings.decisions && view.journey.revision <= 1 && card !== undefined ? (
        <p className="next-for-you" data-testid="walkthrough-intro">
          Start with the decisions that shape this journey. {open} open now; each answer can open more.
        </p>
      ) : null}
      {error === undefined ? null : <p className="callout callout-bad">The frontier could not be read: {error}</p>}
      <Surfaced view={view} keys={surfaced(view.derived.acting_frontier, pass).filter((key) => unlockedBy(pass, key) === undefined || !order.includes(key))} />
      {next === undefined ? <p className="muted small">Reading the frontier...</p> : null}
      {next !== undefined && rows.length === 0 ? (
        <Empty view={view} settings={settings} stalled={next.stalled != null} onContinue={() => void navigate(triagePath(journey, { ...settings, decisions: false }, selected))} />
      ) : null}
      {done ? (
        <div className="callout stack" data-testid="pass-done">
          <span>Every card has been seen once · {passedCount} passed.</span>
          <span className="row">
            <Button onClick={() => { setPass(roundAgain(pass)); }}>Go round again</Button>
            <Button onClick={newPass}>Start a new pass</Button>
          </span>
        </div>
      ) : null}
      {wide ? null : (
        <details className="fold" data-testid="pass-fold">
          <summary>This pass</summary>
          {rail}
        </details>
      )}
      {card === undefined ? null : <PassCard view={view} card={card} place={`${String(Math.min(passedCount + 1, order.length))} of ${String(order.length)}`} by={unlockedBy(pass, card.key)} onPass={onPass} />}
      {wide && !opened ? <Inspector focus={journey} reveal={false}>{rail}</Inspector> : null}
    </section>
  );
}
