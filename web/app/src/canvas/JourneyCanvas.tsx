// The journey canvas (C1 to C7, C15): the journey's level for the detail step and the containers
// collapsed (the ladder, 5.1), read from its local derivation with the next list (rank) and
// "mine", turned into cards and lines, laid out in the layout worker, and drawn; the trace of the
// selected node as an overlay (selecting a node traces it; several selected trace nothing). A card
// opens the node's detail (5.1) beside the canvas, or joins the selection; an edge opens the
// edge's card.
import { Panel } from "@xyflow/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import { nodeDetail, recordOf, titleOf, type GraphNode, type Ready } from "../detail/model.ts";
import { contextMenuOf } from "../detail/offers.ts";
import { pagePath } from "../journeys/address.ts";
import { ViewControls } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { typing } from "../ui/typing.ts";
import { RevealBox, SignalMenu } from "./Chrome.tsx";
import { useFind } from "./find.ts";
import { GraphCanvas, type Reveal } from "./GraphCanvas.tsx";
import { useJoinedTrace, useLaidOut, useProjected } from "./hooks.ts";
import { journeyLooks } from "./journey.ts";
import { ancestorsOf, collapsedAt, currentStages, hiddenCount, kindsAt, leftOutAt, stepOf, stepsFor, withExpanded, withStep } from "./ladder.ts";
import { KINDS, STRENGTH, cardsOf, linesOf, type CanvasModel, type Card, type JourneyExtras, type Level, type Line, type Step } from "./model.ts";
import type { CardActions } from "./NodeCard.tsx";
import { standsFor, traceOverlay } from "./overlay.ts";
import { refitKey } from "./refit.ts";
import type { Selection } from "./selection.ts";
import { canvasPath, edgePath, layoutViewOf, levelRequest, type CanvasView } from "./settings.ts";

/** C1, C2, C5, C6: the journey's canvas model for one level. */
export function journeyModel(ready: Ready, level: Level, extras: JourneyExtras): CanvasModel {
  const graph = ready.journey.graph.nodes ?? [];
  const looks = journeyLooks(ready, extras);
  return { cards: cardsOf(level, graph, looks), lines: linesOf(level, graph, (key) => looks.finished(key)) };
}

/** What a canvas reads from the address and the page: the step it is at, and the containers it collapses. */
function useLadder(ready: Ready, view: CanvasView) {
  return useMemo(() => {
    const nodes = ready.journey.graph.nodes ?? [];
    // Editing structure draws every node, so each can be reached.
    const step: Step = view.edit && view.step === undefined ? "all" : stepOf(view, nodes);
    const active = nodes.filter((node) => recordOf(ready, node).state === "active").map((node) => node.key);
    const current = currentStages(nodes, ready.derived.acting_frontier, active);
    return { nodes, step, steps: stepsFor(nodes), collapsed: collapsedAt(step, nodes, current, view.open, view.shut) };
  }, [ready, view]);
}

/**
 * What Reveal shows beside the view (5.7): the level with every kind and class, less what the view
 * leaves out (a kind the step does not draw, not-relevant or conditional nodes it turned off) and the
 * trace does not touch. The traced nodes the view hides are drawn, ghosted where not relevant.
 */
function onlyTraced(model: CanvasModel, traced: ReadonlySet<string>, view: CanvasView, step: Step, leftOut: ReadonlySet<string>): CanvasModel {
  const left = (card: Card) =>
    leftOut.has(card.key) || !kindsAt(step).includes(card.kind) || (card.journey?.state === "not_relevant" && !view.notRelevant) || (card.journey?.state === "conditional" && !view.undecided);
  const out = new Set(model.cards.filter((card) => left(card) && !standsFor(card).some((key) => traced.has(key))).map((card) => card.key));
  // A line to a card left out runs to the nearest card kept above it, as the view would have drawn it (C2).
  const byKey = new Map(model.cards.map((card) => [card.key, card]));
  const standIn = (key: string): string | undefined => {
    let at = byKey.get(key);
    while (at !== undefined && out.has(at.key)) {
      at = at.parent === undefined ? undefined : byKey.get(at.parent);
    }
    return at?.key;
  };
  const lines = new Map<string, Line>();
  for (const line of model.lines) {
    const [from, to] = [standIn(line.from), standIn(line.to)];
    if (from === undefined || to === undefined || from === to) {
      continue;
    }
    const [id, kept] = [`${from}->${to}`, lines.get(`${from}->${to}`)];
    lines.set(
      id,
      kept === undefined
        ? { ...line, id, from, to }
        : {
            ...kept,
            kind: STRENGTH[line.kind] > STRENGTH[kept.kind] ? line.kind : kept.kind,
            count: kept.count + line.count,
            implicit: kept.implicit && line.implicit,
            gates: kept.gates || line.gates,
            satisfied: kept.satisfied === true && line.satisfied === true,
          },
    );
  }
  return { cards: model.cards.filter((card) => !out.has(card.key)), lines: [...lines.values()] };
}

/**
 * The level less what the step leaves out (Stages: top-level nodes that are not stages or milestones,
 * and what is beneath them), and the lines to them. A card that waited on a node left out says so with
 * a hidden-prerequisites marker, so leaving it out never makes blocked work look free (C2).
 */
function withoutLeftOut(model: CanvasModel, out: ReadonlySet<string>): CanvasModel {
  if (out.size === 0) {
    return model;
  }
  const waiting = new Map<string, string[]>();
  for (const line of model.lines) {
    if (out.has(line.from) && !out.has(line.to) && line.gates && line.satisfied === false) {
      waiting.set(line.to, [...(waiting.get(line.to) ?? []), line.from]);
    }
  }
  const cards = model.cards.filter((card) => !out.has(card.key)).map((card) => (waiting.has(card.key) ? { ...card, hiddenPrerequisites: [...card.hiddenPrerequisites, ...(waiting.get(card.key) ?? [])] } : card));
  return { cards, lines: model.lines.filter((line) => !out.has(line.from) && !out.has(line.to)) };
}

function useJourneyModel(ready: Ready, view: CanvasView, collapsed: string[], step: Step, revealed: ReadonlySet<string> | undefined) {
  const shown = revealed === undefined ? view : { ...view, notRelevant: true, undecided: true };
  const level = useProjected(ready, levelRequest(shown, { kinds: revealed === undefined ? kindsAt(step) : KINDS, collapsed }));
  const next = useProjected(ready, { projection: "next" });
  const mine = useProjected(ready, { projection: "mine" });
  const model = useMemo(() => {
    if (level.value === undefined) {
      return undefined;
    }
    const ranked = (next.value?.items ?? []).map((item) => item.key);
    const entries = mine.value ?? [];
    const whole = journeyModel(ready, level.value, { ranked, mine: entries.map((entry) => entry.node), owned: entries.filter((entry) => entry.kinds.includes("k_owner")).map((entry) => entry.node) });
    const leftOut = leftOutAt(step, ready.journey.graph.nodes ?? []);
    return revealed === undefined ? withoutLeftOut(whole, leftOut) : onlyTraced(whole, revealed, view, step, leftOut);
  }, [ready, level.value, next.value, mine.value, revealed, view, step]);
  // Fading a kind lays nothing out again, so the kinds shown are not part of what the layout is keyed by.
  return { model, error: level.error, layoutView: layoutViewOf({ ...shown, shown: KINDS }, step) };
}

export interface JourneyCanvasProps {
  ready: Ready;
  view: CanvasView;
  selected: string | undefined;
  /** The edge whose card is open, as `<from>~<to>`. */
  edge: string | undefined;
  /** The Decisions highlight: non-decision cards fade; what is drawn does not change. */
  decisions: boolean;
  /** The frontier lens: the nodes kept at full strength while every other card fades; none (or null) fades nothing. */
  emphasis?: ReadonlySet<string> | null | undefined;
  /** In edit mode, a card picked while an edge is drawn ends the edge instead of opening (5.6); true when it took the pick. */
  onPick?: ((key: string) => boolean) | undefined;
  /** The selection: who is selected, and what adds to it. */
  selection: Selection;
}

/** What the Decisions highlight fades: every card that is not a decision or around one. */
function fadedBy(model: CanvasModel, decisions: boolean): ReadonlySet<string> {
  if (!decisions) {
    return new Set();
  }
  const parents = new Map(model.cards.map((card) => [card.key, card.parent]));
  const kept = new Set<string>();
  for (const card of model.cards.filter((each) => each.kind === "decision")) {
    for (let at: string | undefined = card.key; at !== undefined && !kept.has(at); at = parents.get(at)) {
      kept.add(at);
    }
  }
  return new Set(model.cards.map((card) => card.key).filter((key) => !kept.has(key)));
}

/**
 * The selected node's trace. A card standing for hidden nodes (a collapsed stage) traces what they
 * trace (C2, C7), so the line that leaves it is followed; what the card stands for is learned from
 * the drawn canvas, once it is.
 */
function useSelectionTrace(ready: Ready, selected: string | undefined) {
  const own = useProjected(ready, selected === undefined ? undefined : { projection: "trace", key: selected });
  const [stands, setStands] = useState<{ node: string | undefined; keys: string[] }>({ node: undefined, keys: [] });
  const joined = useJoinedTrace(ready, selected, stands.node === selected ? stands.keys : []);
  const learn = useCallback((rolledUp: string[]) => {
    setStands((held) => (held.node === selected && held.keys.join() === rolledUp.join() ? held : { node: selected, keys: rolledUp }));
  }, [selected]);
  return { trace: joined ?? own.value, learn };
}

/** What the canvas draws: the ladder's level laid out, the selected node's trace over it, and what Reveal asks for. */
function useGraph(ready: Ready, view: CanvasView, selected: string | undefined, decisions: boolean, emphasis: ReadonlySet<string> | null | undefined) {
  const journey = ready.journey.header.id;
  const { nodes, step, steps, collapsed } = useLadder(ready, view);
  const { trace, learn } = useSelectionTrace(ready, selected);
  // Reveal (5.7) is a moment, not an address: it ends when something else is selected.
  const [revealedFor, setRevealedFor] = useState<{ node: string | undefined; linked: number } | undefined>(undefined);
  const traced = useMemo(() => (trace === undefined ? undefined : new Set([trace.node, ...trace.upstream, ...trace.downstream])), [trace]);
  const revealed = revealedFor !== undefined && revealedFor.node === selected ? revealedFor : undefined;
  const traceOnly = revealed === undefined ? undefined : traced;
  const open = useMemo(() => (traceOnly === undefined ? [] : [...traceOnly].flatMap((key) => ancestorsOf(nodes, key))), [traceOnly, nodes]);
  const unfolded = useMemo(() => collapsed.filter((key) => !open.includes(key)), [collapsed, open]);
  const { model, error, layoutView } = useJourneyModel(ready, view, unfolded, step, traceOnly);
  const { laidOut, error: layoutError } = useLaidOut(journey, layoutView, model);
  const rolledUp = laidOut?.model.cards.find((card) => card.key === selected)?.rolledUp;
  useEffect(() => {
    if (rolledUp !== undefined) {
      learn(rolledUp);
    }
  }, [rolledUp, learn]);
  const overlay = useMemo(
    () => (selected !== undefined && trace !== undefined && laidOut !== undefined ? traceOverlay(trace, laidOut.model, titleOf(ready, trace.node)) : undefined),
    [selected, trace, laidOut, ready],
  );
  const faded = useMemo(() => (laidOut === undefined ? undefined : fadedBy(laidOut.model, decisions)), [laidOut, decisions]);
  const badges = useMemo(() => {
    const leftOut = leftOutAt(step, nodes);
    const drawn = nodes.filter((node) => !leftOut.has(node.key));
    const state = (key: string) => ready.derived.nodes[key]?.display_state;
    const hiding = (words: string, count: number) => (count === 0 ? [] : [{ words: `${String(count)} ${words}`, count }]);
    const parts = [
      ...hiding("not relevant hidden", view.notRelevant ? 0 : hiddenCount(drawn, (key) => state(key) === "not_relevant", kindsAt(step), collapsed)),
      ...hiding("conditional hidden", view.undecided ? 0 : hiddenCount(drawn, (key) => state(key) === "conditional", kindsAt(step), collapsed)),
    ];
    const filter = parts.length === 0 ? undefined : { count: parts.reduce((sum, part) => sum + part.count, 0), words: parts.map((part) => part.words).join(", ") };
    const own = new Set((laidOut?.model.cards ?? []).map((card) => card.key));
    const kept = (node: GraphNode) => (emphasis === null || emphasis === undefined || emphasis.has(node.key)) && (!decisions || node.kind === "decision");
    const wanted = (emphasis !== null && emphasis !== undefined) || decisions ? nodes.filter(kept) : [];
    // Nodes the Filter hides are its to say; a highlight lists the nodes with no card of their own at this detail (one folded into a container lights the container instead).
    const lost = wanted.filter((node) => !own.has(node.key) && !(state(node.key) === "not_relevant" && !view.notRelevant) && !(state(node.key) === "conditional" && !view.undecided)).length;
    const detail = lost === 0 ? undefined : { count: lost, words: `${String(lost)} hidden at this detail; choose All to see them` };
    return { filter, detail };
  }, [step, nodes, collapsed, view.notRelevant, view.undecided, ready.derived.nodes, laidOut, emphasis, decisions]);
  return { step, steps, trace, laidOut, overlay, faded, badges, revealed: revealed?.linked, error: error ?? layoutError, reveal: (linked: number) => { setRevealedFor({ node: selected, linked }); }, hide: () => { setRevealedFor(undefined); } };
}

/** What a card asks of the canvas: open its node, expand or collapse it (and refit to it, 5.1), trace it. */
function useActions(ready: Ready, view: CanvasView, selected: string | undefined, onPick: JourneyCanvasProps["onPick"], selection: Selection): CardActions {
  const navigate = useNavigate();
  const frame = useFrameActions();
  const journey = ready.journey.header.id;
  return useMemo<CardActions>(
    () => ({
      open: (key) => {
        if (onPick?.(key) !== true) {
          // Picking the node that is already open changes no address, so the inspector must be asked to come back.
          frame?.show("inspector");
          selection.pick(key);
        }
      },
      drill: undefined,
      // The refit rides on the navigation, not the address.
      expand: (key, expanded) => void navigate(canvasPath(journey, withExpanded(view, key, expanded), selected), { state: { reveal: { key, how: expanded ? "expanded" : "collapsed" } } }),
      trace: (key) => { selection.pick(key); },
      title: (key) => titleOf(ready, key),
      menu: {
        groups: (key) => {
          const detail = nodeDetail(ready, key);
          return detail === undefined ? [] : contextMenuOf(ready, detail, selection.members.includes(key)).map((items) => ({ items }));
        },
        // The node's own Actions carry out the choice once it is open beside the canvas.
        choose: (key, id) => {
          if (id === "toggle-selection") {
            selection.toggle(key);
            return;
          }
          selection.release();
          void (id === "show-in-list" ? navigate(pagePath(journey, "list", key)) : navigate(canvasPath(journey, view, key), { replace: key === selected, state: { choice: { node: key, id } } }));
        },
      },
    }),
    [navigate, frame, journey, view, selected, ready, onPick, selection],
  );
}

/** Esc leaves the edit mode, unless a popover is open (it closes first) or the key sheet is. */
function useEscapeEdit(on: boolean, leave: () => void): void {
  useEffect(() => {
    if (!on) {
      return undefined;
    }
    const heard = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && !typing(event.target) && document.querySelector("[data-popover], dialog[open]") === null) {
        event.preventDefault();
        leave();
      }
    };
    // Before the journey's own Esc, which would clear the selected node and leave the mode on.
    document.addEventListener("keydown", heard, true);
    return () => {
      document.removeEventListener("keydown", heard, true);
    };
  }, [on, leave]);
}

/** What the trace reaches that has no card of its own (outside the step, folded into a container, or left out). */
function outsideOf(trace: { upstream: string[]; downstream: string[] } | undefined, model: CanvasModel): number {
  const drawnAs = new Set(model.cards.map((card) => card.key));
  return trace === undefined ? 0 : [...trace.upstream, ...trace.downstream].filter((key) => !drawnAs.has(key)).length;
}

/** The journey's canvas, laid out, with the selected node's trace. */
export function JourneyCanvas({ ready, view, selected, edge, decisions, emphasis, onPick, selection }: JourneyCanvasProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const journey = ready.journey.header.id;
  // One member is inspected and traced; several are only tinted.
  const [single, ...others] = selection.members;
  const alone = others.length === 0 ? single : undefined;
  const graph = useGraph(ready, view, alone, decisions, emphasis);
  const { find, missing } = useFind(ready, view, selection.release);
  const actions = useActions(ready, view, selected, onPick, selection);
  const go = (next: CanvasView, node: string | undefined) => void navigate(canvasPath(journey, next, node));
  useEscapeEdit(view.edit && others.length === 0, useCallback(() => { void navigate(canvasPath(journey, { ...view, edit: false }, selected)); }, [navigate, journey, view, selected]));
  const asked = (location.state as { reveal?: { key: string; how: Reveal["how"] } } | null)?.reveal;
  const reveal = useMemo<Reveal | undefined>(() => (asked === undefined ? undefined : { key: asked.key, how: asked.how, id: location.key }), [asked, location.key]);
  const { laidOut, overlay, trace } = graph;
  if (graph.error !== undefined) {
    return <p className="callout callout-bad">The canvas could not be drawn: {graph.error}</p>;
  }
  if (laidOut === undefined) {
    return <p className="muted small">Laying out the canvas...</p>;
  }
  return (
      <GraphCanvas
        model={laidOut.model}
        layout={laidOut.layout}
        overlay={overlay}
        lens={view.lens}
        faded={graph.faded}
        emphasis={emphasis}
        selected={alone}
        selectedEdge={edge?.replace("~", "->")}
        actions={actions}
        viewKey={refitKey(laidOut, view.edit)}
        label={`${ready.journey.header.name}: canvas`}
        title={ready.journey.header.name}
        onEdge={(line) => void navigate(edgePath(journey, view, line))}
        onBackground={() => {
          if (selection.members.length > 0) {
            selection.clear();
          } else if (edge !== undefined) {
            go(view, undefined);
          }
        }}
        selection={{ picked: new Set(others.length === 0 ? [] : selection.members), members: selection.members, toggle: selection.toggle, set: selection.set }}
        reveal={reveal}
        steps={graph.steps}
        onStep={(next) => { go(withStep(view, next), selected); }}
      >
        <Panel position="top-left" className="canvas-panel canvas-top">
          <ViewControls
            ready={ready}
            screen="graph"
            node={selected}
            detail={{ steps: graph.steps, step: graph.step, onStep: (next) => { go(withStep(view, next), selected); } }}
            badges={graph.badges}
            onFind={find}
          />
          {missing === undefined ? null : (
            <p className="callout" role="status" data-testid="find-missing">
              No node is named like "{missing}".
            </p>
          )}
        </Panel>
        <Panel position="top-right" className="canvas-panel">
          <SignalMenu lens={view.lens} onLens={(lens) => { go({ ...view, lens }, selected); }} />
        </Panel>
        <RevealBox linked={graph.revealed ?? (alone === undefined ? 0 : outsideOf(trace, laidOut.model))} revealed={graph.revealed !== undefined} onReveal={() => { graph.reveal(outsideOf(trace, laidOut.model)); }} onHide={graph.hide} />
      </GraphCanvas>
  );
}
