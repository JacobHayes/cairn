// C11: the walk through, a queue of cards over the acting frontier, one at a time in rank order,
// for "act, or move on"; and its decisions-only mode, the decision walkthrough, which starting a
// journey opens (address.ts, `walkthroughPath`). It is the `walk` view, at Decisions depth for the
// walkthrough, under the toolbar every view shares: its cards are the frontier work the level
// draws at the depth that itself matches the Filter and search (journeys/requests.ts). It ignores
// Depth's Fade: one card at a time has nothing to dim. A card is the
// inspector's own component with Pass on the frame's edge, apart from the node's actions; the
// inspector column holds the pass rail until a linked node is opened. Triage always reads the
// current frontier (the engine's list of next-up work in rank order over the tab's derivation,
// kept current, H6), so an answer that unblocks new nodes surfaces them in the same pass. Pass is client state for this pass only (pass.ts): kept per tab in session
// storage, never sent. What a write made from the pass unlocked (the patch's consequences, never
// a diff of the frontier) comes next, each card labeled with the card that unlocked it.
import { useCallback, useEffect, useMemo, useRef } from "react";

import { stepOf } from "../canvas/ladder.ts";
import type { Step } from "../canvas/model.ts";
import { useDraft } from "../data/drafts.ts";
import { useSaves, useSession } from "../data/react.ts";
import { NodeDetailPanel } from "../detail/NodeDetail.tsx";
import type { Ready } from "../detail/model.ts";
import { walkQueryOf } from "../journeys/requests.ts";
import { useViewState, type ViewState } from "../journeys/view.ts";
import { journeySubject, ResetView, ViewBar } from "../screens/ViewControls.tsx";
import { COLUMN, Inspector, useMedia } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { typing } from "../ui/typing.ts";
import { acted, begin, passedAll, passOn, passOrder, roundAgain, surfaced, unlockedBy, type Pass } from "./pass.ts";
import { PassRail } from "./PassRail.tsx";
import { StalledPanel } from "./Stalled.tsx";
import { WaitingDecisions } from "./Waiting.tsx";
import { useRows } from "./useRows.ts";
import type { NodeRow } from "./why.ts";
import { actingDecisions } from "./waiting.ts";
import "./acting.css";
import { Failed } from "../ui/Failed.tsx";
import { NodeLink } from "../detail/parts.tsx";
import { useViewLevel } from "../canvas/viewLevel.ts";

/** What reached the acting frontier since the pass began, other than what the pass's own actions unlocked and its filters still show (the card's band and the rail name those). */
function Surfaced({ view, keys }: { view: Ready; keys: string[] }) {
  if (keys.length === 0) {
    return null;
  }
  return (
    <p className="callout row" data-testid="surfaced">
      <span>New to act on since this pass began:</span>
      {keys.map((key) => (
        <NodeLink key={key} view={view} node={key} />
      ))}
    </p>
  );
}

/**
 * No card to show: the view's depth, Filter or search draws none of what can be acted on; or, in
 * the walkthrough, no decision can be made by anyone, so what would unblock the next ones; or the
 * journey is stalled (D5); or nothing is left.
 */
function Empty({ view, decisions, stalled, onContinue }: { view: Ready; decisions: boolean; stalled: boolean; onContinue: () => void }) {
  const filtered = decisions ? actingDecisions(view).length > 0 : view.derived.acting_frontier.length > 0;
  if (filtered) {
    return (
      <p className="muted row" data-testid="triage-empty" data-status="filtered">
        Nothing on the frontier is drawn at this depth, filter and search. <ResetView />
      </p>
    );
  }
  if (decisions) {
    return <WaitingDecisions view={view} onContinue={onContinue} />;
  }
  return stalled ? (
    <StalledPanel view={view} />
  ) : (
    <p className="muted" data-testid="triage-empty" data-status="none">
      Nothing is left to act on in this journey.
    </p>
  );
}

/** The journey's pass, begun over its acting frontier the first time triage opens in this tab. */
function usePass(view: Ready): [Pass, (pass: Pass) => void] {
  // A pass kept by an earlier version of the app has no unlocked or acted-on nodes.
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
 * from it: each one's unlocked nodes come next. Writes sent before the pass opened (on another page,
 * even if they land after) and writes in another tab are never heard.
 */
function useUnlocked(journey: string, pass: Pass, setPass: (pass: Pass) => void): void {
  const saves = useSaves();
  const session = useSession();
  const opened = useRef(session.began);
  const heard = useRef(0);
  useEffect(() => {
    const fresh = saves.filter((save) => save.id > heard.current).reverse();
    if (fresh.length === 0) {
      return;
    }
    heard.current = Math.max(...fresh.map((save) => save.id));
    const next = fresh.reduce((now, { unlocked }) => (unlocked?.journey === journey && unlocked.began > opened.current ? acted(now, unlocked.by, unlocked.nodes) : now), pass);
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
              <span className="label">↳ Unlocked by</span> <NodeLink view={view} node={by} />
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

/**
 * C10, C11: the cards: the frontier work in rank order that matches the Filter and search, kept
 * where the level draws it (at Depth's step); and the badges, Depth's counting the cards the
 * step hides. Returns the depth, whose Decisions step is the decision walkthrough.
 */
function useCards(view: Ready, state: ViewState) {
  const nodes = view.journey.graph.nodes ?? [];
  const depth: Step = stepOf({ step: state.depth }, nodes);
  const query = useMemo(() => walkQueryOf(state, depth), [state, depth]);
  const { rows: candidates, error: rowsError } = useRows(view, query);
  const lit = useMemo(() => new Set((candidates ?? []).map((row) => row.key)), [candidates]);
  // The walk draws at the depth whatever Fade says, and counts its candidate cards, not the frontier, against it.
  const { drawn, error: levelError, badges } = useViewLevel({ screen: "walk", state, nodes, selected: undefined, ready: view, lit, held: view });
  const rows = useMemo(() => (candidates ?? []).filter((row) => drawn?.has(row.key) === true), [candidates, drawn]);
  return { depth, rows, read: candidates !== undefined && drawn !== undefined, error: levelError ?? rowsError, badges };
}

/** The walk through: one card at a time, in the order of this pass. */
export function TriageBody({ view, state, selected, onCard }: { view: Ready; state: ViewState; selected: string | undefined; onCard: (key: string | undefined) => void }) {
  const wide = useMedia(COLUMN);
  const journey = view.journey.header.id;
  const [pass, setPass] = usePass(view);
  useUnlocked(journey, pass, setPass);
  const { onState } = useViewState();
  const { depth, rows, read, error, badges } = useCards(view, state);
  const decisions = depth === "decisions";
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
  // The card is the focus node's editor, so the frame opens nothing beside it for the node the address names: it reports the card.
  const cardKey = card?.key;
  useEffect(() => {
    onCard(cardKey);
    return () => { onCard(undefined); };
  }, [cardKey, onCard]);
  const newPass = () => { setPass(begin(view.derived.acting_frontier)); };
  const passedCount = order.filter((key) => pass.passed.includes(key)).length;
  const rail = <PassRail view={view} order={order} pass={pass} onNewPass={newPass} />;
  const opened = selected !== undefined && selected !== card?.key;
  const open = decisions ? actingDecisions(view).length : 0;
  return (
    <>
    <ViewBar subject={journeySubject(view)} screen="walk" badges={badges} />
    <section className="stack pass" aria-label={decisions ? "Decision walkthrough" : "Triage"} data-testid="triage" data-order={order.join(" ")}>
      {decisions && view.journey.revision <= 1 && card !== undefined ? (
        <p className="for-you" data-testid="walkthrough-intro">
          Start with the decisions that shape this journey. {open} open now; each answer can open more.
        </p>
      ) : null}
      {error === undefined ? null : <p className="callout callout-bad"><Failed what="The frontier" error={error} /></p>}
      <Surfaced view={view} keys={surfaced(view.derived.acting_frontier, pass).filter((key) => unlockedBy(pass, key) === undefined || !order.includes(key))} />
      {read ? null : <p className="muted small">Reading the frontier...</p>}
      {read && rows.length === 0 ? <Empty view={view} decisions={decisions} stalled={view.derived.stalled != null} onContinue={() => { onState({ ...state, depth: undefined }, true); }} /> : null}
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
    </>
  );
}
