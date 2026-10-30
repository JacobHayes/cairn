// What authoring reads from a graph, a route draft's or a journey's alike (PRD, How the
// pieces fit: one graph structure): the tree by key, paths, ancestors and descendants, the
// kinds that can hold children (A2), and the edge an author may draw (A3). Pure, so the
// editors and the removal cascade share one reading of the tree.
//
// Cost: an index is one pass over the nodes; every walk is bounded by the graph's size.
import { FILLED_ROLE, LEAF_KINDS, REFERENCE_TARGETS, SLOT_TAKES_ROLE, type Schema } from "@cairn/client";

import { mintKey, slugOf, uniqueId } from "./keys.ts";
import { nodeTitle } from "../ui/gone.ts";

export type Graph = Schema<"Graph">;
export type GraphNode = Schema<"Node">;
export type NodeKind = Schema<"NodeKind">;
export type NodeField = Schema<"NodeField">;
export type AnswerType = Schema<"AnswerType">;
export type ReferenceField = Schema<"ReferenceField">;
export type Mutation = Schema<"Mutation">;
export type Role = Schema<"RoleResolved">;
export type Kind = Schema<"ParticipationKindResolved">;
export type Resource = Schema<"Resource">;
export type Edge = Schema<"Edge">;

/** A kind as a picker offers it: its name in sentence case (DESIGN, Typography), the kind as its value. */
export function kindOption(kind: NodeKind): { value: NodeKind; label: string } {
  return { value: kind, label: `${kind.charAt(0).toUpperCase()}${kind.slice(1)}` };
}

/** A2: whether a node of `kind` holds no children (the engine's table): it is one instant, not a span. */
export function isLeaf(kind: NodeKind): boolean {
  return LEAF_KINDS.includes(kind);
}

/** A2: every kind but the leaves may hold children of any kind. */
export function canHoldChildren(kind: NodeKind): boolean {
  return !isLeaf(kind);
}

const cardinality = (multi: boolean | undefined): "single" | "multi" => (multi === true ? "multi" : "single");

/** A6, A7: the roles that fit a slot of the cardinality `slotMulti` (a participation kind, the default owner), by the engine's table. */
export function rolesForSlot(roles: readonly Role[], slotMulti: boolean): Role[] {
  return roles.filter((role) => SLOT_TAKES_ROLE[cardinality(slotMulti)][cardinality(role.multi)]);
}

/** A6: the roles a decision of `answer` may fill, by the engine's table: none for an answer that fills no role. */
export function rolesForFill(roles: readonly Role[], answer: AnswerType): Role[] {
  const wanted = FILLED_ROLE[answer];
  return roles.filter((role) => cardinality(role.multi) === wanted);
}

/** Nodes in path order that `field` may name (the engine's table of reference targets), for a picker. */
export function nodesFor(tree: Tree, field: ReferenceField): GraphNode[] {
  const { kinds, answer_types: answers } = REFERENCE_TARGETS[field];
  return nodesByPath(tree).filter((node) => kinds.includes(node.kind) && (node.kind !== "decision" || answers === null || answers.includes(node.answer_type ?? "boolean")));
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

/** Keys and references: a node's path, the slash-joined ids from the root. */
export function pathOf(tree: Tree, key: string): string {
  const node = tree.byKey.get(key);
  if (node === undefined) {
    return key;
  }
  return [...ancestorsOf(tree, key), node].map((each) => each.id).join("/");
}

/** A node's title, or words for a node the graph does not hold. */
export function titleIn(tree: Tree, key: string): string {
  return nodeTitle(tree.byKey.get(key)?.title);
}

/** Nodes of `kind` (every kind when none), in path order, for a picker. */
export function nodesByPath(tree: Tree, kind?: NodeKind): GraphNode[] {
  const nodes = [...tree.byKey.values()].filter((node) => kind === undefined || node.kind === kind);
  const paths = new Map(nodes.map((node) => [node.key, pathOf(tree, node.key)]));
  return nodes.sort((left, right) => (paths.get(left.key) ?? "").localeCompare(paths.get(right.key) ?? ""));
}

/** A2: where `key` may move: the root or a container that is neither it nor inside it. */
export function moveTargets(tree: Tree, key: string): GraphNode[] {
  const below = new Set(descendantsOf(tree, key).map((node) => node.key));
  return nodesByPath(tree).filter((node) => canHoldChildren(node.kind) && node.key !== key && !below.has(node.key));
}

/**
 * A new node of `kind` titled `title` under `parent`, with a fresh key and an id unique among its
 * siblings (those in `tree` and any `pending` additions). A decision asks its title as a yes-or-no question.
 */
export function newNode(tree: Tree, kind: NodeKind, title: string, parent: string | undefined, pending: readonly GraphNode[] = []): GraphNode {
  const taken = new Set([...childrenOf(tree, parent).map((child) => child.id), ...pending.filter((node) => (node.parent ?? undefined) === parent).map((node) => node.id)]);
  const node: GraphNode = { key: mintKey("n_"), id: uniqueId(slugOf(title), taken), kind, title: title.trim() };
  const placed = parent === undefined ? node : { ...node, parent };
  return kind === "decision" ? { ...placed, prompt: title.trim(), answer_type: "boolean" } : placed;
}
