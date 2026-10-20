// C9: the List of a route: the nodes the level draws (Depth, Show conditional and the search every
// view of a route shares, journeys/requests.ts), in tree order, each with its date rule in words
// (a route has no state or dates), folded by the ladder with the viewer's own folds, and faded
// below the depth as the other views fade it. A draft's title opens the node's form.
import type { Schema } from "@cairn/client";
import { useMemo } from "react";
import { useNavigate } from "react-router";

import { ruleFoot } from "../authoring/dates.ts";
import { nodesByPath, treeOf, type Tree } from "../authoring/graph.ts";
import type { GraphNode } from "../detail/model.ts";
import { useFolds } from "../canvas/hooks.ts";

import { useNodeAddresses } from "../journeys/address.ts";

import type { ViewState } from "../journeys/view.ts";
import { containerKeys } from "../plan/tree.ts";
import type { LevelRequest } from "../journeys/requests.ts";
import { nodesOf, type Shown } from "./route-reads.ts";
import { useRouteLevel } from "./route-views.ts";
import { TreeFold, TreeGuides } from "../ui/Tree.tsx";
import { NothingMatches, routeSubject, ViewBar } from "./ViewControls.tsx";
import { Failed } from "../ui/Failed.tsx";
import { useViewLevel } from "../canvas/viewLevel.ts";
import { useReviewing } from "../proposals/layer.ts";
import { DIFF_LABELS } from "../proposals/model.ts";

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

/** The drawn nodes in tree order, each under the drawn node it is drawn under, with how far it sits and its tree guides (ui/Tree.tsx); a closed container's nodes are left out. */
function useListRows(tree: Tree, level: Schema<"Level"> | undefined, open: (key: string) => boolean) {
  return useMemo(() => {
    if (level === undefined) {
      return undefined;
    }
    const parents = new Map(level.nodes.map((each) => [each.key, each.parent ?? undefined]));
    const children = new Map<string | undefined, GraphNode[]>();
    for (const node of nodesByPath(tree)) {
      if (parents.has(node.key)) {
        const parent = parents.get(node.key);
        children.set(parent, [...(children.get(parent) ?? []), node]);
      }
    }
    const rows: { node: GraphNode; depth: number; holds: boolean; guides: boolean[] }[] = [];
    const visit = (node: GraphNode, depth: number, guides: boolean[]) => {
      const held = children.get(node.key) ?? [];
      rows.push({ node, depth, holds: held.length > 0, guides });
      if (open(node.key)) {
        held.forEach((child, at) => { visit(child, depth + 1, [...guides, at < held.length - 1]); });
      }
    };
    (children.get(undefined) ?? []).forEach((root) => { visit(root, 0, []); });
    return rows;
  }, [level, tree, open]);
}

/** C14: under a proposal's layer the route as it is, at the same level, supplies the rows the proposal removes, put back where they were; otherwise the route's own tree and level. */
function useWithRemoved(shown: Shown, own: Tree, ownLevel: Schema<"Level"> | undefined, request: LevelRequest) {
  const layer = useReviewing();
  const was = layer?.removed.size === 0 ? undefined : layer?.beforeShown;
  const wasRead = useRouteLevel(was ?? shown, request);
  return useMemo(() => {
    if (layer === undefined || was === undefined || wasRead.level === undefined || ownLevel === undefined) {
      return { tree: own, level: ownLevel };
    }
    const gone = wasRead.level.nodes.filter((entry) => layer.removed.has(entry.key) && !ownLevel.nodes.some((each) => each.key === entry.key));
    const goneNodes = nodesOf(was).filter((node) => gone.some((entry) => entry.key === node.key));
    return { tree: treeOf({ ...shown.graph, nodes: [...(shown.graph.nodes ?? []), ...goneNodes] }), level: { ...ownLevel, nodes: [...ownLevel.nodes, ...gone] } };
  }, [layer, was, wasRead.level, ownLevel, own, shown.graph]);
}

export function RouteList({ shown, state, picks, selected, unanchored, onOpen }: RouteListProps) {
  const addresses = useNodeAddresses();
  const navigate = useNavigate();
  const nodes = nodesOf(shown);
  const layer = useReviewing();
  const own = useMemo(() => treeOf(shown.graph), [shown.graph]);
  const { step, current, request, level: ownLevel, error, dimmed: faded, badges } = useViewLevel({ screen: "list", state, nodes, selected, read: (request, at) => useRouteLevel(shown, request, at) });
  const { open, fold } = useFolds(nodes, current, state.depth, step, picks, selected, (key) => void navigate(addresses.open(key)));
  const { tree, level } = useWithRemoved(shown, own, ownLevel, request);
  const rows = useListRows(tree, level, open);
  const containers = useMemo(() => containerKeys(nodes), [nodes]);
  return (
    <section className="stack list" aria-label="List" data-testid="list">
      <ViewBar subject={routeSubject(nodes)} screen="list" badges={badges} />
      {error === undefined ? null : <p className="callout callout-bad"><Failed what="The list" error={error} /></p>}
      <div className="node-panel list-panel" data-testid="list-panel">
        <ul className="plain-list route-list route-tree" data-testid="route-list">
          {(rows ?? []).map(({ node, depth: indent, holds, guides }) => {
            const foot = ruleFoot(node, tree);
            const diff = layer?.model?.marks[node.key];
            return (
              <li
                key={node.key}
                className="route-list-row"
                data-testid="route-row"
                data-node={node.key}
                data-kind={node.kind}
                data-container={containers.has(node.key)}
                data-diff={diff?.kind}
                data-choice={diff?.choice === true ? true : undefined}
                data-faded={faded(node.key) ? true : undefined}
                data-navigable={onOpen === undefined ? undefined : true}
                aria-current={node.key === selected ? "true" : undefined}
                style={{ paddingInlineStart: `calc(var(--space-2) + var(--space-5) * ${String(indent)})` }}
              >
                <TreeGuides depth={indent} guides={guides} open={holds && open(node.key)} />
                <TreeFold container={holds} open={open(node.key)} label={node.title} testId="row-fold" onFold={() => { fold(node.key); }} />
                {diff === undefined ? null : (
                  <span className="layer-word" data-testid="row-diff" data-diff={diff.kind}>
                    {DIFF_LABELS[diff.kind]}
                    {diff.choice === true ? <span className="layer-choice"> · needs a choice</span> : null}
                  </span>
                )}
                {onOpen === undefined ? (
                  <span className="route-list-title">{node.title}</span>
                ) : (
                  <button type="button" className="link route-list-title" data-testid="row-open" onClick={() => { onOpen(node.key); }}>
                    {node.title}
                  </button>
                )}
                {foot === undefined ? null : <span className="muted small">{foot}</span>}
                {unanchored.has(node.key) ? <span className="node-notice">No path to final</span> : null}
                <span className="node-kind">{node.kind}</span>
              </li>
            );
          })}
        </ul>
        {rows !== undefined && rows.length === 0 ? (
          <div className="empty-row" data-testid="list-empty">
            <NothingMatches />
          </div>
        ) : null}
      </div>
    </section>
  );
}
