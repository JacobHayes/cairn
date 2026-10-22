// A route's canvas (C1 to C7, C15): the shown graph's level for the page's view state (Depth's
// step and the containers collapsed, the Filter and the search, journeys/requests.ts), read from
// the graph's derivation (the worker holds it as a journey in which nothing has happened), turned
// into cards that say their date rule in words, laid out in the layout worker and drawn, with the
// selected node's trace over it, as on a journey: the shared hook (graph.ts) does all of it, and
// this file supplies the route's cards and actions. A container opens and closes in place (C4);
// what Depth's Fade dims is faded. A card, on a draft or a published version, selects its node
// and opens it beside the canvas (a draft's form, a version's read-only panel), or ends an edge
// being drawn; the empty canvas clears the selection. There is no state to light and no signal
// to show on a route, so Frontier and the Signals lens are a journey's alone.
import { useCallback, useMemo } from "react";
import { useLocation, useNavigate } from "react-router";

import { ruleFoot } from "../authoring/dates.ts";
import { titleIn, type Tree, treeOf } from "../authoring/graph.ts";
import { useNodeAddresses } from "../journeys/address.ts";
import { useReviewing } from "../proposals/layer.ts";
import { routePath } from "../routes/address.ts";
import { useRouteBefore } from "../screens/route-views.ts";
import { nodesOf, type Derived, type Shown } from "../screens/route-reads.ts";
import { EmptyPanel, NothingMatches, ResetView, routeSubject, ViewControls } from "../screens/ViewControls.tsx";
import { useFrameActions } from "../shell/frame.tsx";
import { Failed } from "../ui/Failed.tsx";
import { KeyMenu, RevealBox } from "./Chrome.tsx";
import { GraphCanvas } from "./GraphCanvas.tsx";
import { changedKeys, outsideOf, useCanvasGraph, useRevealed, type Side } from "./graph.ts";
import { Panel } from "@xyflow/react";
import { ancestorsOf, withExpanded } from "./ladder.ts";
import { LineInspector, useLineOpening } from "./LineCard.tsx";
import { cardsOf, linesOf, type Level } from "./model.ts";
import type { CardActions } from "./NodeCard.tsx";
import { refitKey } from "./refit.ts";
import { graphSearch, type GraphView } from "./settings.ts";

export interface RouteCanvasProps {
  shown: Derived<Shown>;
  view: GraphView;
  selected: string | undefined;
  /** The line whose card is open, as `<from>~<to>`; the canvas draws it in the inspector. */
  edge: string | undefined;
  /** The nodes with no path to the final milestone (A20): their cards say so. */
  unanchored: ReadonlySet<string>;
  /** A card picked while an edge is drawn ends the edge instead of opening (5.6); true when it took the pick. */
  onPick: ((key: string) => boolean) | undefined;
}

/** The route's canvas model for one level: the cards say their date rule in words and whether the node is cut off from the final milestone. */
function routeSide(shown: Derived<Shown>, tree: Tree, unanchored: ReadonlySet<string>): Side {
  const nodes = nodesOf(shown);
  return {
    held: shown,
    nodes,
    model: (level: Level) => ({
      cards: cardsOf(level, nodes).map((card) => {
        const node = tree.byKey.get(card.key);
        return { ...card, route: { foot: node === undefined ? undefined : ruleFoot(node, tree), unanchored: unanchored.has(card.key) } };
      }),
      lines: linesOf(level, nodes),
      order: level.date_order ?? [],
    }),
  };
}

/** What the canvas draws: the shared graph hook over the route's side, and its before side under a proposal's layer. */
function useGraph(shown: Derived<Shown>, view: GraphView, selected: string | undefined, edge: string | undefined, unanchored: ReadonlySet<string>) {
  const id = shown.route.header.id;
  const layer = useReviewing();
  const tree = useMemo(() => treeOf(shown.graph), [shown.graph]);
  const side = useMemo(() => routeSide(shown, tree, unanchored), [shown, tree, unanchored]);
  // Under a proposal's layer the route as it is, at the same level, supplies what the proposal removes, put back where it was.
  const { before, waiting } = useRouteBefore();
  const wasTree = useMemo(() => (before === undefined ? undefined : treeOf(before.graph)), [before]);
  const was = useMemo(() => (before === undefined || wasTree === undefined ? undefined : routeSide(before, wasTree, new Set())), [before, wasTree]);
  const title = useCallback((key: string) => titleIn(tree, key), [tree]);
  const domain = layer === undefined ? `route:${id}:${String(shown.of)}` : `route:${id}/proposals/${layer.id}`;
  const graph = useCanvasGraph({ view, side, was, wait: waiting, layer, ready: undefined, selected, address: selected, domain, title, edge });
  return { ...graph, tree };
}

/** What a card asks of the canvas: select its node (and bring the inspector back), expand or collapse it in place (and refit to it, 5.1), trace it. */
function useActions(shown: Derived<Shown>, view: GraphView, selected: string | undefined, onPick: RouteCanvasProps["onPick"], tree: Tree, proposal: string | undefined): CardActions {
  const navigate = useNavigate();
  const frame = useFrameActions();
  const { search } = useLocation();
  const addresses = useNodeAddresses();
  const id = shown.route.header.id;
  const version = shown.of === "draft" ? undefined : shown.of;
  const nodes = nodesOf(shown);
  return useMemo<CardActions>(() => {
    const select = (key: string) => {
      if (onPick?.(key) !== true) {
        // Picking the node that is already open changes no address, so the inspector must be asked to come back.
        frame?.show("inspector");
        void navigate(addresses.open(key));
      }
    };
    return {
      open: select,
      // Closing a container that holds the selected node moves the selection to it (C7).
      expand: (key, expanded) => {
        const holding = !expanded && selected !== undefined && ancestorsOf(nodes, selected).includes(key);
        const node = holding ? key : selected;
        void navigate(routePath(id, { version, node, screen: "graph", proposal }, graphSearch(withExpanded(view, key, expanded), search)), { state: { reveal: { key, how: expanded ? "expanded" : "collapsed" } } });
      },
      trace: proposal === undefined ? select : undefined,
      title: (key) => titleIn(tree, key),
      menu: undefined,
    };
  }, [navigate, frame, addresses, id, version, view, search, selected, nodes, tree, onPick, proposal]);
}

export function RouteCanvas({ shown, view, selected, edge, unanchored, onPick }: RouteCanvasProps) {
  const navigate = useNavigate();
  const addresses = useNodeAddresses();
  const { laidOut, overlay, layer, error, faded, badges, tree, trace, line, dates, ...graph } = useGraph(shown, view, selected, edge, unanchored);
  const actions = useActions(shown, view, selected, onPick, tree, layer?.id);
  const reveal = useRevealed(selected);
  const opening = useLineOpening(edge, layer !== undefined);
  // The line's card is the shell's inspector's, drawn from the line this canvas draws; a canvas that failed has nothing to say about one.
  const card = edge === undefined || error !== undefined ? null : <LineInspector focus={`${shown.route.header.id}:${edge}`} line={line} ends={opening.ends} title={(key) => titleIn(tree, key)} has={(key) => tree.byKey.has(key)} />;
  const controls = <ViewControls subject={routeSubject(nodesOf(shown))} screen="graph" badges={badges} />;
  if (error !== undefined) {
    return (
      <div className="stack">
        {controls}
        <p className="callout callout-bad row">
          <Failed what="The canvas" verb="drawn" error={error}>
            <ResetView />
          </Failed>
        </p>
      </div>
    );
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
      firstFit={layer === undefined ? undefined : changedKeys(layer)}
      lens={undefined}
      faded={faded}
      selected={selected}
      dates={dates}
      selectedEdge={opening.selected}
      actions={actions}
      viewKey={refitKey(laidOut, shown.of === "draft" || layer !== undefined)}
      label={`${shown.route.header.name}: canvas`}
      reveal={reveal}
      onEdge={opening.onEdge}
      onBackground={selected === undefined && edge === undefined ? undefined : () => { void navigate(addresses.close()); }}
      held={
        <>
          {controls}
          {laidOut.model.cards.length > 0 ? null : (
            <EmptyPanel testId="nothing-matches">
              {nodesOf(shown).length > 0 ? <NothingMatches /> : shown.of === "draft" ? "The draft has no nodes yet: + Add starts it." : "This version has no nodes."}
            </EmptyPanel>
          )}
        </>
      }
    >
      <Panel position="top-right" className="canvas-panel">
        <KeyMenu />
      </Panel>
      <RevealBox linked={graph.revealed ?? (selected === undefined ? 0 : outsideOf(trace, laidOut.model))} revealed={graph.revealed !== undefined} onReveal={() => { graph.reveal(outsideOf(trace, laidOut.model)); }} onHide={graph.hide} />
    </GraphCanvas>
    </>
  );
}
