// The journey's containment as the Plan list and the timeline read it: each container's children
// in plan order (siblings by schedule, the final milestone last), a node's ancestors, and what is
// done beneath a container. Pure over the local derivation, so the unit tests check it without a page.
//
// Cost: O(nodes) per journey document and per derivation, then O(1) per lookup.
import type { GraphNode, Ready } from "../detail/model.ts";

/** The nodes' containment: roots, then each container's children, siblings in plan order. */
export interface Tree {
  roots: string[];
  children: Map<string, string[]>;
  parents: Map<string, string>;
}

const TREES = new WeakMap<GraphNode[], WeakMap<object, Tree>>();
const NO_NODES: GraphNode[] = [];
const NO_ORDER: string[] = [];

export function treeOf(view: Ready): Tree {
  const nodes = view.journey.graph.nodes ?? NO_NODES;
  // The order is the derivation's, so a new derivation of the same document is a new tree.
  const order = view.derived.plan_order ?? NO_ORDER;
  const derivations = TREES.get(nodes) ?? new WeakMap<object, Tree>();
  TREES.set(nodes, derivations);
  let tree = derivations.get(order);
  if (tree === undefined) {
    const children = new Map<string, string[]>();
    const parents = new Map<string, string>();
    const roots: string[] = [];
    const parentOf = new Map(nodes.map((node) => [node.key, node.parent]));
    // The engine says the order: siblings come in the order they appear in the plan.
    for (const key of order) {
      const parent = parentOf.get(key);
      if (parent == null) {
        roots.push(key);
      } else {
        parents.set(key, parent);
        children.set(parent, [...(children.get(parent) ?? []), key]);
      }
    }
    tree = { roots, children, parents };
    derivations.set(order, tree);
  }
  return tree;
}

/** The nodes that hold others in the graph itself (C2): the one rule for what is a container, whatever a level draws or a search leaves. */
export function containerKeys(nodes: readonly GraphNode[]): ReadonlySet<string> {
  return new Set(nodes.flatMap((node) => (node.parent == null ? [] : [node.parent])));
}

/** `key`'s ancestors, root first. */
export function ancestorsOf(view: Ready, key: string): string[] {
  const { parents } = treeOf(view);
  const ancestors: string[] = [];
  for (let at = parents.get(key); at !== undefined && !ancestors.includes(at); at = parents.get(at)) {
    ancestors.unshift(at);
  }
  return ancestors;
}

/** Every node in plan order: each container before what it holds, as the engine orders them. */
export function planOrder(view: Ready): string[] {
  return view.derived.plan_order ?? NO_ORDER;
}
