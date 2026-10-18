// C9: the List of a route: the nodes the level draws (Depth, Show conditional and the search every
// view of a route shares, journeys/requests.ts), in tree order, each with its date rule in words
// (a route has no state or dates), folded by the ladder with the viewer's own folds, and faded
// below the depth as the other views fade it. A draft's title opens the node's form.
import type { Schema } from "@cairn/client";
import { useMemo } from "react";
import { useNavigate } from "react-router";

import { ruleFoot } from "../authoring/dates.ts";
import { nodesByPath, treeOf, type Tree } from "../authoring/graph.ts";
import { useFolds } from "../canvas/hooks.ts";
import { collapsedAt, drawnStep, stepOf } from "../canvas/ladder.ts";
import { useNodeAddresses } from "../journeys/address.ts";
import { badgesOf, depthFaded, levelRequestOf, outOfFocus, stepHides } from "../journeys/requests.ts";
import type { ViewState } from "../journeys/view.ts";
import { nodesOf, type Shown } from "./route-reads.ts";
import { useRouteLevel } from "./route-views.ts";
import { ResetView, routeSubject, ViewBar } from "./ViewControls.tsx";

const NONE: ReadonlySet<string> = new Set();

export interface RouteListProps {
  shown: Shown;
  state: ViewState;
  /** How many times a depth was picked: a pick drops the viewer's folds. */
  picks: number;
  selected: string | undefined;
  /** The nodes with no path to the final milestone (A20). */
  unanchored: ReadonlySet<string>;
  /** Opens a node, or picks it while an edge is drawn; none where a node opens nothing (a version). */
  onOpen: ((key: string) => void) | undefined;
}

/** The drawn nodes in tree order, each with how far it sits under the drawn nodes above it, less those under a closed one. */
function useListRows(tree: Tree, level: Schema<"Level"> | undefined, open: (key: string) => boolean) {
  return useMemo(() => {
    if (level === undefined) {
      return undefined;
    }
    const parents = new Map(level.nodes.map((each) => [each.key, each.parent ?? undefined]));
    const holders = new Set([...parents.values()].flatMap((parent) => (parent === undefined ? [] : [parent])));
    const chain = (key: string): string[] => {
      const above: string[] = [];
      for (let at = parents.get(key); at !== undefined && !above.includes(at); at = parents.get(at)) {
        above.push(at);
      }
      return above;
    };
    return nodesByPath(tree).flatMap((node) => {
      const above = chain(node.key);
      return parents.has(node.key) && above.every((key) => open(key)) ? [{ node, depth: above.length, holds: holders.has(node.key) }] : [];
    });
  }, [level, tree, open]);
}

export function RouteList({ shown, state, picks, selected, unanchored, onOpen }: RouteListProps) {
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const nodes = nodesOf(shown);
  const tree = useMemo(() => treeOf(shown.graph), [shown.graph]);
  const depth = stepOf({ step: state.depth }, nodes);
  const step = drawnStep(depth, state.fade);
  const request = useMemo(() => levelRequestOf(state, step, []), [state, step]);
  const { level, drawn, error } = useRouteLevel(shown, request, selected);
  const { open, fold } = useFolds(nodes, NONE, state.depth, step, picks, selected, (key) => void navigate(addresses.open(key)));
  const faded = useMemo(() => outOfFocus(undefined, drawn === undefined ? NONE : depthFaded(state, nodes, depth, collapsedAt(depth, nodes, NONE, [], []), drawn)), [state, nodes, depth, drawn]);
  const badges = useMemo(() => badgesOf("list", level?.hidden_by_class ?? {}, undefined, stepHides(nodes, depth, []), state.fade), [level, nodes, depth, state.fade]);
  const rows = useListRows(tree, level, open);
  return (
    <section className="stack list" aria-label="List" data-testid="list">
      <ViewBar subject={routeSubject(nodes)} screen="list" badges={badges} />
      {error === undefined ? null : <p className="callout callout-bad">The list could not be read: {error}</p>}
      <ul className="route-list" data-testid="route-list">
        {(rows ?? []).map(({ node, depth: indent, holds }) => {
          const foot = ruleFoot(node, tree);
          return (
            <li
              key={node.key}
              className="route-list-row"
              data-testid="route-row"
              data-node={node.key}
              data-kind={node.kind}
              data-faded={faded(node.key) ? true : undefined}
              data-navigable={onOpen === undefined ? undefined : true}
              aria-current={node.key === selected ? "true" : undefined}
              style={{ paddingLeft: `calc(var(--space-4) * ${String(indent)})` }}
            >
              {holds ? (
                <button type="button" className="link route-list-fold" aria-expanded={open(node.key)} aria-label={`${open(node.key) ? "Fold" : "Unfold"} ${node.title}`} data-testid="row-fold" onClick={() => { fold(node.key); }}>
                  {open(node.key) ? "▾" : "▸"}
                </button>
              ) : (
                <span className="route-list-fold" aria-hidden="true" />
              )}
              <span className="node-kind">{node.kind}</span>
              {onOpen === undefined ? (
                <span className="route-list-title">{node.title}</span>
              ) : (
                <button type="button" className="link route-list-title" data-testid="row-open" onClick={() => { onOpen(node.key); }}>
                  {node.title}
                </button>
              )}
              {foot === undefined ? null : <span className="muted small">{foot}</span>}
              {unanchored.has(node.key) ? <span className="node-notice">No path to final</span> : null}
            </li>
          );
        })}
      </ul>
      {rows !== undefined && rows.length === 0 ? (
        <p className="muted row" data-testid="list-empty">
          Nothing matches at this depth, filter and search. <ResetView />
        </p>
      ) : null}
    </section>
  );
}
