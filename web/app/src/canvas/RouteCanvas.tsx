// A route's canvas (C1 to C4, C15): the shown graph's level for the page's view state (Depth's
// step and the containers collapsed, the Filter and the search, journeys/requests.ts), derived
// in the tab with no journey state, turned into cards that say their date rule in words, laid
// out in the layout worker and drawn. A container opens and closes in place (C4), as on a
// journey; what Depth's Fade dims is faded. A draft's card opens the node's form beside the
// canvas (5.6) or ends an edge being drawn; a version's opens nothing. The controls float at
// its top left, the same ones every view of a route has.
import { useMemo } from "react";
import { useLocation, useNavigate } from "react-router";

import { ruleFoot } from "../authoring/dates.ts";
import { titleIn, type Tree, treeOf } from "../authoring/graph.ts";
import { journeyLayoutView } from "../journeys/requests.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { routePath } from "../routes/address.ts";
import { useRouteLevel } from "../screens/route-views.ts";
import { nodesOf, type Shown } from "../screens/route-reads.ts";
import { EmptyPanel, NothingMatches, ResetView, routeSubject, ViewControls } from "../screens/ViewControls.tsx";
import { useNodeAddresses } from "../journeys/address.ts";
import { GraphCanvas } from "./GraphCanvas.tsx";
import { useLaidOut } from "./hooks.ts";
import { changedKeys, useRevealed } from "./JourneyCanvas.tsx";
import { ancestorsOf, withExpanded } from "./ladder.ts";
import { cardsOf, linesOf, type CanvasModel } from "./model.ts";
import type { CardActions } from "./NodeCard.tsx";
import { refitKey } from "./refit.ts";
import { graphSearch, type GraphView } from "./settings.ts";
import { Failed } from "../ui/Failed.tsx";
import { useReviewing, withRemoved, type Composed } from "../proposals/layer.ts";
import { marksOverlay } from "./overlay.ts";
import { useViewLevel } from "./viewLevel.ts";


export interface RouteCanvasProps {
  shown: Shown;
  view: GraphView;
  selected: string | undefined;
  /** The nodes with no path to the final milestone (A20): their cards say so. */
  unanchored: ReadonlySet<string>;
  /** Whether a card opens a node (a draft's does). */
  editable: boolean;
  /** A card picked while an edge is drawn ends the edge instead of opening (5.6); true when it took the pick. */
  onPick: ((key: string) => boolean) | undefined;
}

/** What the canvas draws: the level as cards laid out, what Depth's Fade dims, and what the badges say. */
function useGraph(shown: Shown, view: GraphView, selected: string | undefined, unanchored: ReadonlySet<string>) {
  const id = shown.route.header.id;
  const nodes = nodesOf(shown);
  const tree = useMemo(() => treeOf(shown.graph), [shown.graph]);
  const layer = useReviewing();
  // The node the address names is always drawn (C7): the level reads it in when the view's own does not.
  // Under a proposal's layer the route as it is, at the same level, supplies what the proposal removes, put back where it was.
  const was = layer?.removed.size === 0 ? undefined : layer?.beforeShown;
  const asked = selected !== undefined && was !== undefined && !nodes.some((node) => node.key === selected) ? undefined : selected;
  const { depth, request, level, error, faded, badges } = useViewLevel({ screen: "graph", state: view.state, nodes, selected: asked, folds: view, read: (request, at) => useRouteLevel(shown, request, at) });
  const wasRead = useRouteLevel(was ?? shown, request);
  const wasTree = useMemo(() => (was === undefined ? undefined : treeOf(was.graph)), [was]);
  const composed = useMemo<Composed | undefined>(() => {
    if (level === undefined) {
      return undefined;
    }
    const cardsFor = (from: typeof level, within: typeof nodes, by: Tree) =>
      cardsOf(from, within).map((card) => {
        const node = by.byKey.get(card.key);
        return { ...card, route: { foot: node === undefined ? undefined : ruleFoot(node, by), unanchored: unanchored.has(card.key) } };
      });
    const kept: CanvasModel = { cards: cardsFor(level, nodes, tree), lines: linesOf(level, nodes) };
    if (layer?.model === undefined || was === undefined || wasTree === undefined || wasRead.level === undefined) {
      return { model: kept, removedLines: new Set(), shrunk: new Map() };
    }
    const gone: CanvasModel = { cards: cardsFor(wasRead.level, nodesOf(was), wasTree), lines: linesOf(wasRead.level, nodesOf(was)) };
    return withRemoved(kept, gone, layer.removed, layer.model.diff.edgesRemoved);
  }, [level, nodes, tree, unanchored, layer, was, wasTree, wasRead.level]);
  const model = composed?.model;
  const laid = useLaidOut(layer === undefined ? `route:${id}:${String(shown.of)}` : `route:${id}/proposals/${layer.id}`, journeyLayoutView(view.state, depth), model);
  const laidOut = valueOf(laid);
  const overlay = useMemo(() => {
    if (layer?.model === undefined || laidOut === undefined) {
      return undefined;
    }
    return marksOverlay("What the proposal changes", layer.model.marks, laidOut.model, { added: layer.model.diff.edgesAdded, removed: layer.model.diff.edgesRemoved }, composed);
  }, [layer, laidOut, composed]);
  return { laidOut, overlay, layer, error: error ?? errorWords(laid), faded, badges, tree };
}

/** What a card asks of the canvas: open its node (a draft's), expand or collapse it in place (and refit to it, 5.1). */
function useActions(shown: Shown, view: GraphView, selected: string | undefined, editable: boolean, onPick: RouteCanvasProps["onPick"], tree: Tree, proposal: string | undefined): CardActions {
  const navigate = useNavigate();
  const { search } = useLocation();
  const addresses = useNodeAddresses();
  const id = shown.route.header.id;
  const version = shown.of === "draft" ? undefined : shown.of;
  const nodes = nodesOf(shown);
  return useMemo<CardActions>(
    () => ({
      open: editable
        ? (key) => {
            if (onPick?.(key) !== true) {
              void navigate(addresses.open(key));
            }
          }
        : undefined,
      // Closing a container that holds the selected node moves the selection to it (C7).
      expand: (key, expanded) => {
        const holding = !expanded && selected !== undefined && ancestorsOf(nodes, selected).includes(key);
        const node = editable ? (holding ? key : selected) : undefined;
        void navigate(routePath(id, { version, node, screen: "graph", proposal }, graphSearch(withExpanded(view, key, expanded), search)), { state: { reveal: { key, how: expanded ? "expanded" : "collapsed" } } });
      },
      trace: undefined,
      title: (key) => titleIn(tree, key),
      menu: undefined,
    }),
    [navigate, addresses, id, version, view, search, selected, nodes, tree, editable, onPick, proposal],
  );
}

export function RouteCanvas({ shown, view, selected, unanchored, editable, onPick }: RouteCanvasProps) {
  const { laidOut, overlay, layer, error, faded, badges, tree } = useGraph(shown, view, selected, unanchored);
  const actions = useActions(shown, view, selected, editable, onPick, tree, layer?.id);
  const reveal = useRevealed(selected);
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
    return <p className="muted small">Laying out the canvas...</p>;
  }
  return (
    <GraphCanvas
      model={laidOut.model}
      layout={laidOut.layout}
      overlay={overlay}
      firstFit={layer === undefined ? undefined : changedKeys(layer)}
      lens={undefined}
      faded={faded}
      selected={selected}
      actions={actions}
      viewKey={refitKey(laidOut, editable)}
      label={`${shown.route.header.name}: canvas`}
      reveal={reveal}
      held={
        <>
          {controls}
          {laidOut.model.cards.length > 0 ? null : (
            <EmptyPanel testId="nothing-matches">
              <NothingMatches />
            </EmptyPanel>
          )}
        </>
      }
    />
  );
}
