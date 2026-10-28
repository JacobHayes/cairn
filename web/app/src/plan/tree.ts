// The journey's containment as the Plan list and the timeline read it: each container's children
// in plan order (siblings by schedule, the final milestone last), a node's ancestors, and what is
// done beneath a container. Pure over the local derivation, so the unit tests check it without a page.
//
// Cost: O(nodes log nodes) per journey document and per derivation, then O(1) per lookup.
import { recordOf, type GraphNode, type Ready } from "../detail/model.ts";

/** The nodes' containment: roots, then each container's children, siblings in plan order. */
export interface Tree {
  roots: string[];
  children: Map<string, string[]>;
  parents: Map<string, string>;
}

const TREES = new WeakMap<GraphNode[], WeakMap<object, Tree>>();
const NO_NODES: GraphNode[] = [];

/**
 * The soonest date a node has: when it can start, when it is due, or when it was finished. A
 * snoozed node is not now: its date is the later of that and the day its snooze lifts, and
 * undated when the hold on it has no date yet, so it sorts by when it comes back.
 */
function ownDate(view: Ready, node: GraphNode): string | undefined {
  const derived = view.derived.nodes[node.key];
  const dates = derived?.dates;
  const current = soonestOf([dates?.earliest_start?.date, dates?.due?.date, recordOf(view, node).finished_on ?? undefined]);
  if (derived?.display_state !== "snoozed") {
    return current;
  }
  // The engine says the day the holding snooze lifts; none yet means the hold has no date, so the row is undated.
  const lifts = derived.snooze_lifts ?? undefined;
  return lifts === undefined ? undefined : current === undefined || current < lifts ? lifts : current;
}

function soonestOf(dates: readonly (string | undefined)[]): string | undefined {
  return dates.reduce<string | undefined>((soonest, date) => (date !== undefined && (soonest === undefined || date < soonest) ? date : soonest), undefined);
}

/**
 * The order of siblings: soonest first (a container by the soonest thing beneath it), the undated
 * after the dated, the final milestone last of all, and key order between any two that tie.
 */
function planSort(view: Ready, nodes: readonly GraphNode[], children: Map<string, string[]>): (left: string, right: string) => number {
  const byNode = new Map(nodes.map((node) => [node.key, node]));
  const soonest = new Map<string, string | undefined>();
  const scheduleOf = (key: string): string | undefined => {
    if (!soonest.has(key)) {
      const node = byNode.get(key);
      soonest.set(key, soonestOf([node === undefined ? undefined : ownDate(view, node), ...(children.get(key) ?? []).map(scheduleOf)]));
    }
    return soonest.get(key);
  };
  const final = (key: string) => Number(byNode.get(key)?.final === true);
  const compare = <T>(left: T, right: T) => (left < right ? -1 : left > right ? 1 : 0);
  return (left, right) => {
    const [from, to] = [scheduleOf(left), scheduleOf(right)];
    return final(left) - final(right) || Number(from === undefined) - Number(to === undefined) || (from === undefined || to === undefined ? 0 : compare(from, to)) || compare(left, right);
  };
}

export function treeOf(view: Ready): Tree {
  const nodes = view.journey.graph.nodes ?? NO_NODES;
  // The order reads the derivation's dates, so a new derivation of the same document is a new tree.
  const derivations = TREES.get(nodes) ?? new WeakMap<object, Tree>();
  TREES.set(nodes, derivations);
  let tree = derivations.get(view.derived.nodes);
  if (tree === undefined) {
    const children = new Map<string, string[]>();
    const parents = new Map<string, string>();
    const roots: string[] = [];
    for (const node of nodes) {
      if (node.parent == null) {
        roots.push(node.key);
      } else {
        parents.set(node.key, node.parent);
        children.set(node.parent, [...(children.get(node.parent) ?? []), node.key]);
      }
    }
    const order = planSort(view, nodes, children);
    roots.sort(order);
    for (const kids of children.values()) {
      kids.sort(order);
    }
    tree = { roots, children, parents };
    derivations.set(view.derived.nodes, tree);
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

/** Every node in plan order: each container before what it holds. */
export function planOrder(view: Ready): string[] {
  const { roots, children } = treeOf(view);
  const order: string[] = [];
  const visit = (key: string) => {
    order.push(key);
    (children.get(key) ?? []).forEach(visit);
  };
  roots.forEach(visit);
  return order;
}
