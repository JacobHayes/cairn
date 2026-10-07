// What authoring reads from a graph, a route draft's or a journey's alike (PRD, How the
// pieces fit: one graph structure): the tree by key, paths, ancestors and descendants, the
// kinds that can hold children (A2), and the edge an author may draw (A3). Pure, so the
// editors and the removal cascade share one reading of the tree.
//
// Cost: an index is one pass over the nodes; every walk is bounded by the graph's size.
import type { Schema } from "@cairn/client";

export type Graph = Schema<"Graph">;
export type GraphNode = Schema<"Node">;
export type NodeKind = Schema<"NodeKind">;
export type NodeField = Schema<"NodeField">;
export type AnswerType = Schema<"AnswerType">;
export type Mutation = Schema<"Mutation">;
export type Role = Schema<"RoleResolved">;
export type Kind = Schema<"ParticipationKindResolved">;
export type Resource = Schema<"Resource">;
export type Edge = Schema<"Edge">;

/** The fixed set of kinds (PRD glossary), in the order a palette offers them. */
export const KINDS: readonly NodeKind[] = ["group", "deliverable", "action", "decision", "milestone"];

/** A2: decisions and milestones are leaves; every other kind may hold children of any kind. */
export function canHoldChildren(kind: NodeKind): boolean {
  return kind !== "decision" && kind !== "milestone";
}

/** A graph's nodes by key and by parent, built once per graph. */
export interface Tree {
  graph: Graph;
  byKey: ReadonlyMap<string, GraphNode>;
  /** Children by parent key; the roots under `""`. */
  children: ReadonlyMap<string, readonly GraphNode[]>;
}

const ROOT = "";

export function treeOf(graph: Graph): Tree {
  const byKey = new Map<string, GraphNode>();
  const children = new Map<string, GraphNode[]>();
  for (const node of graph.nodes ?? []) {
    byKey.set(node.key, node);
    const parent = node.parent ?? ROOT;
    children.set(parent, [...(children.get(parent) ?? []), node]);
  }
  return { graph, byKey, children };
}

/** A node's children, or the roots for none. */
export function childrenOf(tree: Tree, key: string | undefined): readonly GraphNode[] {
  return tree.children.get(key ?? ROOT) ?? [];
}

/** A node's ancestors, root first; a broken chain stops where it breaks. */
export function ancestorsOf(tree: Tree, key: string): GraphNode[] {
  const ancestors: GraphNode[] = [];
  let parent = tree.byKey.get(key)?.parent ?? undefined;
  while (parent !== undefined && ancestors.length < tree.byKey.size) {
    const found = tree.byKey.get(parent);
    if (found === undefined) {
      break;
    }
    ancestors.unshift(found);
    parent = found.parent ?? undefined;
  }
  return ancestors;
}

/** Every node beneath `key`, parents before their children. */
export function descendantsOf(tree: Tree, key: string): GraphNode[] {
  const found: GraphNode[] = [];
  const queue = [...childrenOf(tree, key)];
  while (queue.length > 0 && found.length < tree.byKey.size) {
    const next = queue.shift();
    if (next !== undefined) {
      found.push(next);
      queue.push(...childrenOf(tree, next.key));
    }
  }
  return found;
}

/** Whether `ancestor` contains `node`, at any depth. */
export function isAncestor(tree: Tree, ancestor: string, node: string): boolean {
  return ancestorsOf(tree, node).some((each) => each.key === ancestor);
}

/** Identity and references: a node's path, the slash-joined ids from the root. */
export function pathOf(tree: Tree, key: string): string {
  const node = tree.byKey.get(key);
  if (node === undefined) {
    return key;
  }
  return [...ancestorsOf(tree, key), node].map((each) => each.id).join("/");
}

/** A node's title, or its key when the graph does not hold it. */
export function titleIn(tree: Tree, key: string): string {
  return tree.byKey.get(key)?.title ?? key;
}

/** Nodes of `kind` (every kind when none), in path order, for a picker. */
export function nodesByPath(tree: Tree, kind?: NodeKind): GraphNode[] {
  const nodes = [...tree.byKey.values()].filter((node) => kind === undefined || node.kind === kind);
  const paths = new Map(nodes.map((node) => [node.key, pathOf(tree, node.key)]));
  return nodes.sort((left, right) => (paths.get(left.key) ?? "").localeCompare(paths.get(right.key) ?? ""));
}

/** Why `node` cannot require `requires` (A3), or undefined when the edge may be drawn. */
export function edgeRefusal(tree: Tree, node: string, requires: string): string | undefined {
  if (node === requires) {
    return "A node cannot require itself.";
  }
  if (isAncestor(tree, requires, node)) {
    return "It sits inside that node: containment already relates them, so an explicit edge to an ancestor is refused (A3).";
  }
  if (isAncestor(tree, node, requires)) {
    return "That node sits inside it: a parent already requires its children, so an explicit edge to a descendant is refused (A3).";
  }
  if ((tree.byKey.get(node)?.requires ?? []).includes(requires)) {
    return "It already requires that node.";
  }
  return undefined;
}

/** A2: where `key` may move: the root or a container that is neither it nor inside it. */
export function moveTargets(tree: Tree, key: string): GraphNode[] {
  const below = new Set(descendantsOf(tree, key).map((node) => node.key));
  return nodesByPath(tree).filter((node) => canHoldChildren(node.kind) && node.key !== key && !below.has(node.key));
}
