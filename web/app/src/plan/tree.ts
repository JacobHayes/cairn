// The journey's containment as the Plan list and the timeline read it: each container's children
// in plan order (siblings by schedule, the final milestone last), a node's ancestors, and what is
// done beneath a container. Pure over the local derivation, so the unit tests check it without a page.
//
// Cost: O(nodes log nodes) per journey document and per derivation, then O(1) per lookup.
import { nodeOf, recordOf, type GraphNode, type Ready } from "../detail/model.ts";

/** The nodes' containment: roots, then each container's children, siblings in plan order. */
export interface Tree {
  roots: string[];
  children: Map<string, string[]>;
  parents: Map<string, string>;
}

const TREES = new WeakMap<GraphNode[], WeakMap<object, Tree>>();
const NO_NODES: GraphNode[] = [];

/** The soonest date a node has: when it can start, when it is due, or when it was finished. */
function ownDate(view: Ready, node: GraphNode): string | undefined {
  const dates = view.derived.nodes[node.key]?.dates;
  return soonestOf([dates?.earliest_start?.date, dates?.due?.date, recordOf(view, node).finished_on ?? undefined]);
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

/** What lies beneath a container: the work still counted (not ruled out, not skipped), how much is done, and what needs attention. */
export interface Roll {
  done: number;
  total: number;
  overdue: number;
  toDecide: number;
}

const NOTHING: Roll = { done: 0, total: 0, overdue: 0, toDecide: 0 };

function plus(left: Roll, right: Roll): Roll {
  return { done: left.done + right.done, total: left.total + right.total, overdue: left.overdue + right.overdue, toDecide: left.toDecide + right.toDecide };
}

const ROLLS = new WeakMap<object, Map<string, Roll>>();

/** Each container's roll-up over the work beneath it, read from the derivation's display states. */
export function rollups(view: Ready): Map<string, Roll> {
  const cached = ROLLS.get(view.derived.nodes);
  if (cached !== undefined) {
    return cached;
  }
  const { roots, children } = treeOf(view);
  const rolls = new Map<string, Roll>();
  // A group is only what it holds; a deliverable or action with children is work of its own too.
  const own = (key: string): Roll => {
    const derived = view.derived.nodes[key];
    const state = derived?.display_state;
    if (derived === undefined || state === "not_relevant" || state === "skipped") {
      return NOTHING;
    }
    return {
      done: state === "done" ? 1 : 0,
      total: 1,
      overdue: derived.overdue ? 1 : 0,
      toDecide: state === "ready" && nodeOf(view, key)?.kind === "decision" ? 1 : 0,
    };
  };
  const visit = (key: string): Roll => {
    const kids = children.get(key) ?? [];
    if (kids.length === 0) {
      return own(key);
    }
    const inside = kids.map(visit).reduce(plus, NOTHING);
    rolls.set(key, inside);
    return nodeOf(view, key)?.kind === "group" ? inside : plus(inside, own(key));
  };
  roots.forEach(visit);
  ROLLS.set(view.derived.nodes, rolls);
  return rolls;
}
