// The journey canvas (C1 to C7, C15): the journey's level for the view state (Depth's step
// and the containers collapsed, the ladder, 5.1; the relevance classes, the Filter and the
// search, journeys/requests.ts), read from its local derivation with the next list (rank) and
// "mine", turned into cards and lines, laid out in the layout worker, and drawn; the trace of the
// selected node as an overlay (selecting a node traces it; several selected trace nothing), and
// what Frontier lights at full strength with the rest faded, as is what Depth's Fade dims (the
// level is then the Actions one, so nothing moves). A card opens the
// node's detail (5.1) beside the canvas, or joins the selection; an edge opens the edge's card.
import { Panel } from "@xyflow/react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router";

import { nodeDetail, titleOf, type Ready } from "../detail/model.ts";
import { contextMenuOf } from "../detail/offers.ts";
import { screenSwitched } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { badgesOf, currentStagesOf, depthFaded, journeyLayoutView, levelRequestOf, litOf, stepHides, type LevelRequest } from "../journeys/requests.ts";
import { EmptyPanel, journeySubject, NothingMatches, ResetView, ViewControls } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { overlayOpen, typing } from "../ui/typing.ts";
import { RevealBox, SignalMenu } from "./Chrome.tsx";
import { GraphCanvas, type Reveal } from "./GraphCanvas.tsx";
import { errorWords, valueOf, type ReadView } from "../data/keyed-read.ts";
import { useJoinedTrace, useLaidOut, useLevel, useProjected } from "./hooks.ts";
import { journeyLooks } from "./journey.ts";
import { ancestorsOf, collapsedAt, drawnStep, stepOf, withExpanded } from "./ladder.ts";
import { KINDS, STRENGTH, cardsOf, linesOf, type CanvasModel, type JourneyExtras, type Level, type Line, type Step } from "./model.ts";
import type { CardActions } from "./NodeCard.tsx";
import { standsFor, traceOverlay } from "./overlay.ts";
import { refitKey } from "./refit.ts";
import type { Selection } from "./selection.ts";
import { graphPath, type GraphView } from "./settings.ts";

/** C1, C2, C5, C6: the journey's canvas model for one level. */
export function journeyModel(ready: Ready, level: Level, extras: JourneyExtras): CanvasModel {
  const graph = ready.journey.graph.nodes ?? [];
  const looks = journeyLooks(ready, extras);
  return { cards: cardsOf(level, graph, looks), lines: linesOf(level, graph, (key) => looks.finished(key)) };
}

/**
 * What a canvas reads from the address and the page: the depth it is at, the step it draws (the
 * depth, or Actions under Fade) with the containers that draws collapses, and the containers
 * Hide at the depth would collapse (what Fade dims is measured against them).
 */
function useLadder(ready: Ready, view: GraphView) {
  return useMemo(() => {
    const nodes = ready.journey.graph.nodes ?? [];
    const depth: Step = stepOf({ step: view.state.depth }, nodes);
    const drawn = drawnStep(depth, view.state.fade);
    const current = currentStagesOf(ready);
    return { nodes, depth, drawn, collapsed: collapsedAt(drawn, nodes, current, view.open, view.shut), hidden: collapsedAt(depth, nodes, current, view.open, view.shut) };
  }, [ready, view.state.depth, view.state.fade, view.open, view.shut]);
}

/**
 * What Reveal shows beside the view (5.7): the level with every kind and class and no filter or
 * search, less what the view does not draw (`drawn`, the view's own level under the same
 * collapse) and the trace does not touch. The traced nodes the view hides are drawn, ghosted
 * where not relevant.
 */
function onlyTraced(model: CanvasModel, traced: ReadonlySet<string>, drawn: ReadonlySet<string>): CanvasModel {
  const out = new Set(model.cards.filter((card) => !drawn.has(card.key) && !standsFor(card).some((key) => traced.has(key))).map((card) => card.key));
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

/** Whether a read has neither a value nor a failure to show yet. */
const isPending = (read: ReadView<unknown>) => read.status === "idle" || read.status === "loading";

/** The view's level (C2), and while Reveal is on, the whole journey's beside it under the same collapse, less what neither draws nor the trace reaches. */
function useJourneyModel(ready: Ready, view: GraphView, collapsed: string[], drawn: Step, depth: Step, revealed: ReadonlySet<string> | undefined, selected: string | undefined) {
  const request = useMemo(() => levelRequestOf(view.state, drawn, collapsed), [view.state, drawn, collapsed]);
  const everything = useMemo<LevelRequest | undefined>(() => (revealed === undefined ? undefined : { projection: "level", shown: KINDS, ...(collapsed.length === 0 ? {} : { collapsed }) }), [revealed, collapsed]);
  // The node the address names is always drawn (C7): the level reads it in when the view's own does not.
  const { level, error: levelError } = useLevel(ready, request, selected);
  const wholeRead = useProjected(ready, everything);
  const whole = valueOf(wholeRead);
  const mineRead = useProjected(ready, { projection: "mine" });
  const mine = valueOf(mineRead);
  // The cards say what is the viewer's, so the first canvas waits for it: a layout made without it would be laid out again, and fitted before it.
  const answered = !isPending(mineRead);
  const model = useMemo(() => {
    const shown = revealed === undefined ? level : whole;
    if (level === undefined || shown === undefined || !answered) {
      return undefined;
    }
    const entries = mine ?? [];
    const drawn = journeyModel(ready, shown, { mine: entries.map((entry) => entry.node), owned: entries.filter((entry) => entry.kinds.includes("k_owner")).map((entry) => entry.node) });
    return revealed === undefined ? drawn : onlyTraced(drawn, revealed, new Set(level.nodes.map((node) => node.key)));
  }, [ready, level, whole, mine, answered, revealed]);
  // Frontier, Fade's dimming and the viewer's expands and collapses keep the layout's positions; the step drawn, the Filter and the search lay it out afresh.
  return { model, level, error: levelError ?? errorWords(wholeRead), layoutView: journeyLayoutView(view.state, depth) };
}

export interface JourneyCanvasProps {
  ready: Ready;
  view: GraphView;
  selected: string | undefined;
  /** The edge whose card is open, as `<from>~<to>`. */
  edge: string | undefined;
  /** In edit mode, a card picked while an edge is drawn ends the edge instead of opening (5.6); true when it took the pick. */
  onPick?: ((key: string) => boolean) | undefined;
  /** The selection: who is selected, and what adds to it. */
  selection: Selection;
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
  return { trace: joined ?? valueOf(own), learn };
}

/** What the canvas draws: the view's level laid out, the selected node's trace over it, what the highlights light, and what Reveal asks for. */
function useGraph(ready: Ready, view: GraphView, selected: string | undefined, address: string | undefined) {
  const journey = ready.journey.header.id;
  const { nodes, depth, drawn, collapsed, hidden } = useLadder(ready, view);
  const { trace, learn } = useSelectionTrace(ready, selected);
  // Reveal (5.7) is a moment, not an address: it ends when something else is selected.
  const [revealedFor, setRevealedFor] = useState<{ node: string | undefined; linked: number } | undefined>(undefined);
  const traced = useMemo(() => (trace === undefined ? undefined : new Set([trace.node, ...trace.upstream, ...trace.downstream])), [trace]);
  const revealed = revealedFor !== undefined && revealedFor.node === selected ? revealedFor : undefined;
  const traceOnly = revealed === undefined ? undefined : traced;
  const open = useMemo(() => (traceOnly === undefined ? [] : [...traceOnly].flatMap((key) => ancestorsOf(nodes, key))), [traceOnly, nodes]);
  const unfolded = useMemo(() => collapsed.filter((key) => !open.includes(key)), [collapsed, open]);
  const { model, level, error, layoutView } = useJourneyModel(ready, view, unfolded, drawn, depth, traceOnly, address);
  const laidRead = useLaidOut(journey, layoutView, model);
  const laidOut = valueOf(laidRead);
  const layoutError = errorWords(laidRead);
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
  const lit = useMemo(() => litOf("graph", view.state, ready), [view.state, ready]);
  // Revealed nodes are drawn beside the level's, and Depth's fade dims them as it does the rest.
  const faded = useMemo(
    () => (level === undefined ? undefined : depthFaded(view.state, nodes, depth, hidden, new Set([...level.nodes.map((node) => node.key), ...(traceOnly ?? [])]))),
    [view.state, level, nodes, depth, hidden, traceOnly],
  );
  const badges = useMemo(() => badgesOf("graph", level?.hidden_by_class ?? {}, lit, stepHides(nodes, depth, hidden), view.state.fade), [level, lit, nodes, depth, hidden, view.state.fade]);
  return { depth, trace, faded, laidOut, overlay, lit, badges, revealed: revealed?.linked, error: error ?? layoutError, reveal: (linked: number) => { setRevealedFor({ node: selected, linked }); }, hide: () => { setRevealedFor(undefined); } };
}

/** What a card asks of the canvas: open its node, expand or collapse it (and refit to it, 5.1), trace it. */
function useActions(ready: Ready, view: GraphView, selected: string | undefined, onPick: JourneyCanvasProps["onPick"], selection: Selection): CardActions {
  const navigate = useNavigate();
  const frame = useFrameActions();
  const { search } = useLocation();
  const target = useJourneyTarget();
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
      // The refit rides on the navigation, not the address.
      // Closing a container that holds the selected node moves the selection to it (C7).
      expand: (key, expanded) => {
        const holding = !expanded && selected !== undefined && ancestorsOf(ready.journey.graph.nodes ?? [], selected).includes(key);
        void navigate(graphPath(journey, withExpanded(view, key, expanded), { node: holding ? key : selected }, search), { state: { reveal: { key, how: expanded ? "expanded" : "collapsed" } } });
      },
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
          void (id === "show-in-list" ? navigate(screenSwitched(target, search, "list", key)) : navigate(graphPath(journey, view, { node: key }, search), { replace: key === selected, state: { choice: { node: key, id } } }));
        },
      },
    }),
    [navigate, frame, target, search, journey, view, selected, ready, onPick, selection],
  );
}

/** Esc leaves the edit mode, unless a popover is open (it closes first) or the key sheet is. */
function useEscapeEdit(on: boolean, leave: () => void): void {
  useEffect(() => {
    if (!on) {
      return undefined;
    }
    const heard = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && !typing(event.target) && !overlayOpen()) {
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

/**
 * Where the canvas is asked to go: what a navigation asked for (its `reveal` state), else, while
 * the location is still the one the canvas mounted at, the node its address names, centred once. Any
 * navigation changes the location, so an opening request is gone for good and never comes back to
 * centre the first node after a later reveal.
 */
export function useRevealed(selected: string | undefined): Reveal | undefined {
  const location = useLocation();
  const [opened] = useState(() => ({ key: location.key, node: selected }));
  const asked = (location.state as { reveal?: { key: string; how: Reveal["how"] } } | null)?.reveal;
  return useMemo<Reveal | undefined>(() => {
    if (asked !== undefined) {
      return { key: asked.key, how: asked.how, id: location.key };
    }
    return opened.key === location.key && opened.node !== undefined ? { key: opened.node, how: "centre", id: location.key } : undefined;
  }, [asked, opened, location.key]);
}

/** The journey's canvas, laid out, with the selected node's trace. */
export function JourneyCanvas({ ready, view, selected, edge, onPick, selection }: JourneyCanvasProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const journey = ready.journey.header.id;
  // One member is inspected and traced; several are only tinted.
  const [single, ...others] = selection.members;
  const alone = others.length === 0 ? single : undefined;
  const graph = useGraph(ready, view, alone, selected);
  const actions = useActions(ready, view, selected, onPick, selection);
  const go = (next: GraphView, node: string | undefined) => void navigate(graphPath(journey, next, { node }, location.search));
  useEscapeEdit(view.edit && others.length === 0, useCallback(() => { void navigate(graphPath(journey, { ...view, edit: false }, { node: selected }, location.search)); }, [navigate, journey, view, selected, location.search]));
  const reveal = useRevealed(selected);
  const { laidOut, overlay, trace } = graph;
  if (graph.error !== undefined) {
    // The controls stay, so a setting the engine refused can be changed or reset from here.
    return (
      <div className="stack">
        <ViewControls subject={journeySubject(ready)} screen="graph" badges={graph.badges} />
        <p className="callout callout-bad row">
          The canvas could not be drawn: {graph.error} <ResetView />
        </p>
      </div>
    );
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
        emphasis={graph.lit}
        faded={graph.faded}
        selected={alone}
        selectedEdge={edge?.replace("~", "->")}
        actions={actions}
        viewKey={refitKey(laidOut, view.edit)}
        label={`${ready.journey.header.name}: canvas`}
        onEdge={(line) => void navigate(graphPath(journey, view, { edge: `${line.from}~${line.to}` }, location.search))}
        onBackground={() => {
          if (selection.members.length > 0) {
            selection.clear();
          } else if (edge !== undefined) {
            go(view, undefined);
          }
        }}
        selection={{ picked: new Set(others.length === 0 ? [] : selection.members), members: selection.members, toggle: selection.toggle, set: selection.set }}
        reveal={reveal}
        held={
          <>
            <ViewControls subject={journeySubject(ready)} screen="graph" badges={graph.badges} />
            {laidOut.model.cards.length > 0 ? null : (
              <EmptyPanel testId="nothing-matches">
                <NothingMatches />
              </EmptyPanel>
            )}
          </>
        }
      >
        <Panel position="top-right" className="canvas-panel">
          <SignalMenu lens={view.lens} onLens={(lens) => { go({ ...view, lens }, selected); }} />
        </Panel>
        <RevealBox linked={graph.revealed ?? (alone === undefined ? 0 : outsideOf(trace, laidOut.model))} revealed={graph.revealed !== undefined} onReveal={() => { graph.reveal(outsideOf(trace, laidOut.model)); }} onHide={graph.hide} />
      </GraphCanvas>
  );
}
