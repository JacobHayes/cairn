// The journey canvas (C1 to C7, C15): the journey's level for the view state (Depth's step
// and the containers collapsed, the ladder, 5.1; the relevance classes, the Filter and the
// search, journeys/requests.ts), read from its local derivation with the next list (rank) and
// "mine", turned into cards and lines, laid out in the layout worker, and drawn; the trace of the
// selected node as an overlay (selecting a node traces it; several selected trace nothing), and
// what Frontier lights at full strength with the rest faded, as is what Depth's Fade dims (the
// level is then the Actions one, so nothing moves). A card opens the
// node's detail (5.1) beside the canvas, or joins the selection; a line opens its card (LineCard.tsx).
import { OWNER_KIND } from "@cairn/client";
import { Panel } from "@xyflow/react";
import { useCallback, useEffect, useMemo } from "react";
import { useLocation, useNavigate } from "react-router";

import { nodeDetail, nodeOf, type GraphNode, type Ready, titleOf } from "../detail/model.ts";
import { contextMenuOf } from "../detail/offers.ts";
import { screenSwitched } from "../journeys/address.ts";
import { useJourneyTarget } from "../journeys/useJourneyTarget.ts";
import { useReviewing } from "../proposals/layer.ts";
import { useWritable } from "../screens/writable.ts";
import { EmptyPanel, journeySubject, NothingMatches, ResetView, ViewControls } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { overlayOpen, typing } from "../ui/typing.ts";
import { RevealBox, SignalMenu } from "./Chrome.tsx";
import { GraphCanvas } from "./GraphCanvas.tsx";
import { valueOf, type ReadView } from "../data/keyed-read.ts";
import { outsideOf, useCanvasGraph, useRevealed, changedKeys } from "./graph.ts";
import { useProjected } from "./hooks.ts";
import { journeyLooks } from "./journey.ts";
import { ancestorsOf, withExpanded } from "./ladder.ts";
import { LineInspector, useLineOpening } from "./LineCard.tsx";
import { cardsOf, linesOf, type CanvasModel, type JourneyExtras, type Level } from "./model.ts";
import type { CardActions } from "./NodeCard.tsx";
import { refitKey } from "./refit.ts";
import type { Selection } from "./selection.ts";
import { graphPath, type GraphView } from "./settings.ts";
import { Failed } from "../ui/Failed.tsx";

/** C1, C2, C5, C6: the journey's canvas model for one level. */
export function journeyModel(ready: Ready, level: Level, extras: JourneyExtras): CanvasModel {
  const graph = ready.journey.graph.nodes ?? [];
  const looks = journeyLooks(ready, extras);
  return { cards: cardsOf(level, graph, looks), lines: linesOf(level, graph, (edge) => looks.waiting(edge)) };
}

/** Whether a read has neither a value nor a failure to show yet. */
const isPending = (read: ReadView<unknown>) => read.status === "idle" || read.status === "loading";

const NO_NODES: readonly GraphNode[] = [];

/** What the canvas draws: the shared graph hook over the journey's side, and the journey's before side under a proposal's layer. */
function useGraph(ready: Ready, view: GraphView, selected: string | undefined, address: string | undefined, edge: string | undefined) {
  const journey = ready.journey.header.id;
  const layer = useReviewing();
  const nodes = ready.journey.graph.nodes ?? NO_NODES;
  const mineRead = useProjected(ready, { projection: "mine" });
  const mine = valueOf(mineRead);
  // The cards say what is the viewer's, so the first canvas waits for it: a layout made without it would be laid out again, and fitted before it.
  const answered = !isPending(mineRead);
  const model = useMemo(() => {
    if (!answered) {
      return undefined;
    }
    const entries = mine ?? [];
    const extras = { mine: entries.map((entry) => entry.node), owned: entries.filter((entry) => entry.kinds.includes(OWNER_KIND.key)).map((entry) => entry.node) };
    return (level: Level) => journeyModel(ready, level, extras);
  }, [ready, mine, answered]);
  const side = useMemo(() => ({ held: ready, nodes, model }), [ready, nodes, model]);
  const before = layer?.before;
  const was = useMemo(() => (before === undefined ? undefined : { held: before, nodes: before.journey.graph.nodes ?? NO_NODES, model: (level: Level) => journeyModel(before, level, { mine: [], owned: [] }) }), [before]);
  const title = useCallback((key: string) => titleOf(ready, key), [ready]);
  return useCanvasGraph({ view, side, was, layer, ready, selected, address, domain: layer === undefined ? journey : `${journey}/proposals/${layer.id}`, title, edge });
}

export interface JourneyCanvasProps {
  ready: Ready;
  view: GraphView;
  selected: string | undefined;
  /** The line whose card is open, as `<from>~<to>`. */
  edge: string | undefined;
  /** In edit mode, a card picked while an edge is drawn ends the edge instead of opening (5.6); true when it took the pick. */
  onPick?: ((key: string) => boolean) | undefined;
  /** The selection: who is selected, and what adds to it. */
  selection: Selection;
}

/** What a card asks of the canvas: open its node, expand or collapse it (and refit to it, 5.1), trace it. */
function useActions(ready: Ready, view: GraphView, selected: string | undefined, onPick: JourneyCanvasProps["onPick"], selection: Selection): CardActions {
  const navigate = useNavigate();
  const frame = useFrameActions();
  const { search } = useLocation();
  const target = useJourneyTarget();
  const journey = ready.journey.header.id;
  const layer = useReviewing();
  const writable = useWritable(ready);
  const proposal = target.proposal;
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
        void navigate(graphPath(journey, withExpanded(view, key, expanded), { node: holding ? key : selected, proposal }, search), { state: { reveal: { key, how: expanded ? "expanded" : "collapsed" } } });
      },
      trace: layer === undefined ? (key) => { selection.pick(key); } : undefined,
      title: (key) => titleOf(ready, key),
      // Acting on a picture of the future would be wrong: the layer's canvas offers no menu.
      menu: layer !== undefined ? undefined : {
        groups: (key) => {
          const detail = nodeDetail(ready, key);
          return detail === undefined ? [] : contextMenuOf(ready, detail, writable, selection.members.includes(key)).map((items) => ({ items }));
        },
        // The node's own Actions carry out the choice once it is open beside the canvas.
        choose: (key, id) => {
          if (id === "toggle-selection") {
            selection.toggle(key);
            return;
          }
          selection.release();
          void (id === "show-in-list" ? navigate(screenSwitched(target, search, "list", key)) : navigate(graphPath(journey, view, { node: key, proposal }, search), { replace: key === selected, state: { choice: { node: key, id } } }));
        },
      },
    }),
    [navigate, frame, target, search, journey, view, selected, ready, onPick, selection, layer, writable, proposal],
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

/** The canvas could not be drawn; the controls stay, so a setting the engine refused can be changed or reset from here. */
function CanvasFailed({ ready, badges, error }: { ready: Ready; badges: ReturnType<typeof useGraph>["badges"]; error: string }) {
  return (
    <div className="stack">
      <ViewControls subject={journeySubject(ready)} screen="graph" badges={badges} />
      <p className="callout callout-bad row">
        <Failed what="The canvas" verb="drawn" error={error}>
          <ResetView />
        </Failed>
      </p>
    </div>
  );
}

/** The journey's canvas, laid out, with the selected node's trace. */
export function JourneyCanvas({ ready, view, selected, edge, onPick, selection }: JourneyCanvasProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const journey = ready.journey.header.id;
  // One member is inspected and traced; several are only tinted.
  const [single, ...others] = selection.members;
  const alone = others.length === 0 ? single : undefined;
  const graph = useGraph(ready, view, alone, selected, edge);
  const line = useLineOpening(edge, graph.layer !== undefined);
  const actions = useActions(ready, view, selected, onPick, selection);
  const proposal = useJourneyTarget().proposal;
  const go = (next: GraphView, node: string | undefined) => void navigate(graphPath(journey, next, { node, proposal }, location.search));
  useEscapeEdit(view.edit && others.length === 0, useCallback(() => { void navigate(graphPath(journey, { ...view, edit: false }, { node: selected }, location.search)); }, [navigate, journey, view, selected, location.search]));
  const reveal = useRevealed(selected);
  const { laidOut, overlay, trace } = graph;
  // The line's card is the shell's inspector's, drawn from the line this canvas draws; a canvas that failed has nothing to say about one.
  const card = edge === undefined || graph.error !== undefined ? null : <LineInspector focus={`${journey}:${edge}`} line={graph.line} ends={line.ends} title={(key) => titleOf(ready, key)} has={(key) => nodeOf(ready, key) !== undefined} />;
  if (graph.error !== undefined) {
    return <CanvasFailed ready={ready} badges={graph.badges} error={graph.error} />;
  }
  if (laidOut === undefined) {
    return <><p className="muted small">Laying out the canvas...</p>{card}</>;
  }
  return (
    <>
      {card}
      <GraphCanvas
        model={laidOut.model}
        layout={laidOut.layout}
        overlay={overlay}
        lens={view.lens}
        emphasis={graph.lit}
        faded={graph.faded}
        selected={alone}
        dates={graph.dates}
        selectedEdge={line.selected}
        actions={actions}
        viewKey={refitKey(laidOut, view.edit)}
        label={`${ready.journey.header.name}: canvas`}
        onEdge={line.onEdge}
        onBackground={() => {
          if (selection.members.length > 0) {
            selection.clear();
          } else if (edge !== undefined) {
            go(view, undefined);
          }
        }}
        selection={graph.layer === undefined ? { picked: new Set(others.length === 0 ? [] : selection.members), members: selection.members, toggle: selection.toggle, set: selection.set } : { picked: new Set(), members: selection.members, toggle: () => undefined, set: () => undefined }}
        firstFit={graph.layer === undefined ? undefined : changedKeys(graph.layer)}
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
    </>
  );
}
