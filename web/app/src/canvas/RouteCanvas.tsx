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
import { treeOf, type Tree } from "../authoring/graph.ts";
import { depthFaded, journeyLayoutView, levelRequestOf, stepHides, badgesOf } from "../journeys/requests.ts";
import { errorWords, valueOf } from "../data/keyed-read.ts";
import { routePath } from "../routes/address.ts";
import { useRouteLevel } from "../screens/route-views.ts";
import { nodesOf, type Shown } from "../screens/route-reads.ts";
import { ResetView, routeSubject, ViewControls } from "../screens/ViewControls.tsx";
import { useNodeAddresses } from "../journeys/address.ts";
import { GraphCanvas } from "./GraphCanvas.tsx";
import { useLaidOut } from "./hooks.ts";
import { useRevealed } from "./JourneyCanvas.tsx";
import { ancestorsOf, collapsedAt, drawnStep, stepOf, withExpanded } from "./ladder.ts";
import { cardsOf, linesOf, type CanvasModel } from "./model.ts";
import type { CardActions } from "./NodeCard.tsx";
import { refitKey } from "./refit.ts";
import { graphSearch, type GraphView } from "./settings.ts";

const NONE: ReadonlySet<string> = new Set();

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
  const depth = stepOf({ step: view.state.depth }, nodes);
  const drawn = drawnStep(depth, view.state.fade);
  const collapsed = useMemo(() => collapsedAt(drawn, nodes, NONE, view.open, view.shut), [drawn, nodes, view.open, view.shut]);
  const hidden = useMemo(() => collapsedAt(depth, nodes, NONE, view.open, view.shut), [depth, nodes, view.open, view.shut]);
  const request = useMemo(() => levelRequestOf(view.state, drawn, collapsed), [view.state, drawn, collapsed]);
  // The node the address names is always drawn (C7): the level reads it in when the view's own does not.
  const { level, error } = useRouteLevel(shown, request, selected);
  const model = useMemo<CanvasModel | undefined>(() => {
    if (level === undefined) {
      return undefined;
    }
    const cards = cardsOf(level, nodes).map((card) => {
      const node = tree.byKey.get(card.key);
      return { ...card, route: { foot: node === undefined ? undefined : ruleFoot(node, tree), unanchored: unanchored.has(card.key) } };
    });
    return { cards, lines: linesOf(level, nodes) };
  }, [level, nodes, tree, unanchored]);
  const laid = useLaidOut(`route:${id}:${String(shown.of)}`, journeyLayoutView(view.state, depth), model);
  const faded = useMemo(() => (level === undefined ? undefined : depthFaded(view.state, nodes, depth, hidden, new Set(level.nodes.map((node) => node.key)))), [view.state, level, nodes, depth, hidden]);
  const badges = useMemo(() => badgesOf("graph", level?.hidden_by_class ?? {}, undefined, stepHides(nodes, depth, hidden), view.state.fade), [level, nodes, depth, hidden, view.state.fade]);
  return { laidOut: valueOf(laid), error: error ?? errorWords(laid), faded, badges, tree };
}

/** What a card asks of the canvas: open its node (a draft's), expand or collapse it in place (and refit to it, 5.1). */
function useActions(shown: Shown, view: GraphView, selected: string | undefined, editable: boolean, onPick: RouteCanvasProps["onPick"], tree: Tree): CardActions {
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
        void navigate(routePath(id, { version, node, screen: "graph" }, graphSearch(withExpanded(view, key, expanded), search)), { state: { reveal: { key, how: expanded ? "expanded" : "collapsed" } } });
      },
      trace: undefined,
      title: (key) => tree.byKey.get(key)?.title ?? key,
      menu: undefined,
    }),
    [navigate, addresses, id, version, view, search, selected, nodes, tree, editable, onPick],
  );
}

export function RouteCanvas({ shown, view, selected, unanchored, editable, onPick }: RouteCanvasProps) {
  const { laidOut, error, faded, badges, tree } = useGraph(shown, view, selected, unanchored);
  const actions = useActions(shown, view, selected, editable, onPick, tree);
  const reveal = useRevealed(selected);
  const controls = <ViewControls subject={routeSubject(nodesOf(shown))} screen="graph" badges={badges} />;
  if (error !== undefined) {
    return (
      <div className="stack">
        {controls}
        <p className="callout callout-bad row">
          The canvas could not be drawn: {error} <ResetView />
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
      overlay={undefined}
      lens={undefined}
      faded={faded}
      selected={selected}
      actions={actions}
      viewKey={refitKey(laidOut, editable)}
      label={`${shown.route.header.name}: canvas`}
      title={shown.route.header.name}
      reveal={reveal}
      held={
        <>
          {controls}
          {laidOut.model.cards.length > 0 ? null : (
            <p className="callout row" role="status" data-testid="nothing-matches">
              Nothing matches the depth, filter and search. <ResetView />
            </p>
          )}
        </>
      }
    />
  );
}
