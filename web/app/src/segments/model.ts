// B13, C19: what inserting a segment asks of a person and what it sends, worked out from the
// graph it goes into and the segment version it copies. Everything here is pure: the stepper
// draws it and the engine does the insertion (every mapping mistake is a violation of the
// graph the patch produces; this only chooses sensible defaults and the `omit` that the
// two-filling-decisions case needs, so the usual insertion needs no repair).
import type { Schema } from "@cairn/client";

import { decisionsIn } from "../authoring/condition.ts";
import { childrenOf, type Graph, type GraphNode, type Kind, type Mutation, type Role, type Tree } from "../authoring/graph.ts";
import { mintKey, slugOf, uniqueId } from "../authoring/keys.ts";
import type { CanvasModel } from "../canvas/model.ts";
import { DIFF_LABELS, type DiffMark } from "../proposals/model.ts";

type Lineage = Schema<"Lineage">;
type Insertion = Schema<"Insertion">;

/** What a role or kind becomes: one the graph has, or a new one. */
export type Mapping = "add" | { existing: string };

/** What the person chose in the stepper's second and third steps; what they left alone takes its default. */
export interface Choices {
  /** The container the root goes under; none for the top level. */
  parent: string | undefined;
  /** The root's title; empty takes the segment's own. */
  title: string;
  /** Graph nodes the root will require (Starts after). */
  after: readonly string[];
  /** Graph nodes that will require the root (Comes before). */
  before: readonly string[];
  /** Per segment role and kind, what the person picked. */
  roles: Readonly<Record<string, Mapping>>;
  kinds: Readonly<Record<string, Mapping>>;
}

export const NO_CHOICES: Choices = { parent: undefined, title: "", after: [], before: [], roles: {}, kinds: {} };

/** The segment's one root (a published segment has exactly one). */
export function rootOf(segment: Graph): GraphNode | undefined {
  return (segment.nodes ?? []).find((node) => node.parent == null);
}

/**
 * C19: the mutations that put a segment draft's top-level nodes, and any `joining` it is about to
 * add, under one new group named for the segment (a segment has one top-level node, so a second
 * needs the first under a group; offered when the first is a leaf, else the second goes under it).
 * One patch, so the rule holds at its end.
 */
export function wrapRoots(roots: readonly GraphNode[], name: string, joining: readonly GraphNode[] = []): Mutation[] {
  const group: GraphNode = { key: mintKey("n_"), id: uniqueId(slugOf(name), new Set([...roots, ...joining].map((node) => node.id))), kind: "group", title: name };
  return [
    { op: "add_node", node: group },
    ...roots.map((node): Mutation => ({ op: "set_node_field", node: node.key, value: { parent: group.key } })),
    ...joining.map((node): Mutation => ({ op: "add_node", node: { ...node, parent: group.key } })),
  ];
}

/** "4 steps": how many nodes inserting it adds. */
export function sizeWords(segment: Graph): string {
  const count = (segment.nodes ?? []).length;
  return `${String(count)} ${count === 1 ? "step" : "steps"}`;
}

/** What fills a role in the graph: a decision whose answer does (E3), or a direct fill kept in a journey's state. */
export type Filling = { by: GraphNode } | "direct";

export function fillingOf(host: Graph, role: string): Filling | undefined {
  const by = (host.nodes ?? []).find((node) => node.fills_role === role);
  if (by !== undefined) {
    return { by };
  }
  return (host.state?.role_fills?.[role] ?? []).length > 0 ? "direct" : undefined;
}

/** One row of the People step: a segment role or kind, what it can become, and what that means for the segment's own filling decision. */
export interface PeopleRow {
  type: "role" | "kind";
  /** The segment's key for it. */
  key: string;
  title: string;
  /** What it becomes, by default or as chosen. */
  mapping: Mapping;
  /** The graph's roles or kinds it may be mapped onto: those of the same cardinality (a mismatch is not offered). */
  options: readonly { key: string; title: string }[];
  /** Roles only: the graph already fills the chosen role, and the segment's decisions that fill it are left out. */
  filled: { by: string | undefined; leftOut: readonly GraphNode[] } | undefined;
}

export interface Plan {
  mutation: Extract<Mutation, { op: "insert_segment" }>;
  rows: readonly PeopleRow[];
  /** The root as it will be named: its title, and the id it takes among its new siblings. */
  root: { title: string; id: string; idTaken: boolean };
}

type Named = Role | Kind;

const titleOf = (item: Named) => item.title ?? item.id;
const same = (left: Named, right: Named) => (left.multi ?? false) === (right.multi ?? false);

/** The mapping an untouched row takes: the graph's role or kind with the same id and cardinality, else a new one. */
function defaultMapping(item: Named, candidates: readonly Named[]): Mapping {
  const match = candidates.find((each) => each.id === item.id && same(each, item));
  return match === undefined ? "add" : { existing: match.key };
}

/** Whether a node of the segment reads `decision` in a condition, so leaving the decision out would dangle. */
function readInCondition(segment: Graph, decision: string): boolean {
  return (segment.nodes ?? []).some((node) => node.relevant_when != null && decisionsIn(node.relevant_when).includes(decision));
}

/**
 * C19: the People step's rows and the `omit` they imply. A role mapped onto one the graph
 * already fills leaves out the segment's own filling decisions, unless a segment condition
 * reads one, in which case an untouched row defaults to a new role; a row the person
 * points at such a role anyway keeps the decision, and the engine says so in its preview.
 */
export function peopleRows(host: Graph, segment: Graph, choices: Pick<Choices, "roles" | "kinds">): PeopleRow[] {
  const roles = (segment.roles ?? []).map((role): PeopleRow => {
    const candidates = (host.roles ?? []).filter((each) => same(each, role));
    const deciders = (segment.nodes ?? []).filter((node) => node.fills_role === role.key);
    const blocked = deciders.some((node) => readInCondition(segment, node.key));
    const typical = defaultMapping(role, host.roles ?? []);
    const filledByDefault = typical !== "add" && fillingOf(host, typical.existing) !== undefined;
    const mapping = choices.roles[role.key] ?? (filledByDefault && blocked ? "add" : typical);
    const filling = mapping === "add" ? undefined : fillingOf(host, mapping.existing);
    const leaving = filling === undefined || blocked ? [] : deciders;
    return {
      type: "role",
      key: role.key,
      title: titleOf(role),
      mapping,
      options: candidates.map((each) => ({ key: each.key, title: titleOf(each) })),
      filled: filling === undefined || leaving.length === 0 ? undefined : { by: filling === "direct" ? undefined : filling.by.title, leftOut: leaving },
    };
  });
  const kinds = (segment.participation_kinds ?? []).map(
    (kind): PeopleRow => ({
      type: "kind",
      key: kind.key,
      title: titleOf(kind),
      mapping: choices.kinds[kind.key] ?? defaultMapping(kind, host.participation_kinds ?? []),
      options: (host.participation_kinds ?? []).filter((each) => same(each, kind)).map((each) => ({ key: each.key, title: titleOf(each) })),
      filled: undefined,
    }),
  );
  return [...roles, ...kinds];
}

/**
 * C19, B13: the `insert_segment` mutation for `choices`: the roots' wiring as edges (the root
 * requires each Starts after node; each Comes before node requires the root), every role and
 * kind mapped explicitly, and the segment's filling decisions left out where the graph's own
 * fills the role. `host` is the graph it goes into, `tree` its index.
 */
export function planInsertion(host: Graph, tree: Tree, segment: Graph, lineage: Lineage, insertion: string, choices: Choices): Plan {
  const root = rootOf(segment);
  const rows = peopleRows(host, segment, choices);
  const omit = rows.flatMap((row) => (row.filled === undefined ? [] : row.filled.leftOut.map((node) => node.key)));
  const rootKey = root?.key ?? "";
  const rootTitle = choices.title.trim() === "" ? (root?.title ?? "") : choices.title.trim();
  const siblings = new Set(childrenOf(tree, choices.parent).map((node) => node.id));
  const id = uniqueId(root?.id ?? "", siblings);
  const mappings = (type: PeopleRow["type"]) =>
    Object.fromEntries(rows.filter((row) => row.type === type).map((row) => [row.key, row.mapping === "add" ? ("add" as const) : { existing: row.mapping.existing }]));
  const mutation: Plan["mutation"] = {
    op: "insert_segment",
    insertion,
    segment: lineage,
    ...(choices.parent === undefined ? {} : { parent: choices.parent }),
    ...(choices.title.trim() === "" ? {} : { root_title: rootTitle }),
    roles: mappings("role"),
    kinds: mappings("kind"),
    omit,
    edges: [
      ...choices.after.map((host) => ({ node: { segment: rootKey }, requires: { host } })),
      ...choices.before.map((host) => ({ node: { host }, requires: { segment: rootKey } })),
    ],
  };
  return { mutation, rows, root: { title: rootTitle, id, idTaken: id !== root?.id } };
}

/** The insertions a graph holds that another does not: what an insertion proposal adds. */
export function newInsertions(before: Graph | null | undefined, after: Graph | null | undefined): Insertion[] {
  const held = new Set((before?.insertions ?? []).map((insertion) => insertion.key));
  return (after?.insertions ?? []).filter((insertion) => !held.has(insertion.key));
}

/** The member of `insertion` that sits at its top: the one whose parent is not a member. */
export function insertionRoot(graph: Graph, insertion: Insertion): string | undefined {
  const members = new Set(Object.keys(insertion.nodes));
  return (graph.nodes ?? []).find((node) => members.has(node.key) && !(node.parent != null && members.has(node.parent)))?.key;
}

/** "⧉ Security review v2": the segment and version, as a card's foot and a list row say it. */
export function originWords(name: string, version: number): string {
  return `⧉ ${name} v${String(version)}`;
}

/**
 * C19: the insertions `after` adds, each as the words that name it on its root card and as the
 * members that belong to it, so review and the stepper's preview mark them the same way.
 */
export function insertionOrigins(before: Graph | null | undefined, after: Graph | null | undefined, nameOf: (route: string) => string): { roots: Record<string, string>; members: Record<string, string> } {
  const roots: Record<string, string> = {};
  const members: Record<string, string> = {};
  for (const insertion of newInsertions(before, after)) {
    const words = originWords(nameOf(insertion.segment.route), insertion.segment.version);
    const root = after === null || after === undefined ? undefined : insertionRoot(after, insertion);
    if (root !== undefined) {
      roots[root] = words;
    }
    for (const key of Object.keys(insertion.nodes)) {
      members[key] = words;
    }
  }
  return { roots, members };
}

/** C19: a canvas's cards, with an insertion's root saying which segment and version it is in its foot (a journey's card keeps its date elsewhere). */
export function withOriginFoots(model: CanvasModel, roots: Readonly<Record<string, string>>): CanvasModel {
  if (Object.keys(roots).length === 0) {
    return model;
  }
  const cards = model.cards.map((card) => {
    const words = roots[card.key];
    if (words === undefined) {
      return card;
    }
    return card.journey === undefined ? { ...card, route: { foot: words, unanchored: card.route?.unanchored ?? false } } : { ...card, journey: { ...card.journey, foot: { words, tone: "plain" as const } } };
  });
  return { ...model, cards };
}

/** The marks of a diff with each added member saying which insertion it belongs to, on the tag's hover and in the list's foot. */
export function noteOrigins(marks: Record<string, DiffMark>, members: Record<string, string>): Record<string, DiffMark> {
  return Object.fromEntries(Object.entries(marks).map(([key, mark]) => [key, members[key] !== undefined && mark.label === DIFF_LABELS.added ? { ...mark, note: members[key] } : mark]));
}

/** The roles `after` has that `before` does not: what an insertion adds besides nodes. */
export function addedRoles(before: Graph | null | undefined, after: Graph | null | undefined): Role[] {
  const held = new Set((before?.roles ?? []).map((role) => role.key));
  return (after?.roles ?? []).filter((role) => !held.has(role.key));
}

/** C8: where a member came from, as the inspector's Origin says it. */
export interface MemberOrigin {
  insertion: Insertion;
  /** Whether the member is the insertion's root. */
  root: boolean;
}

/** The insertion holding node `key`, if any. */
export function memberOrigin(graph: Graph, key: string): MemberOrigin | undefined {
  const insertion = (graph.insertions ?? []).find((each) => key in each.nodes);
  return insertion === undefined ? undefined : { insertion, root: insertionRoot(graph, insertion) === key };
}

/** Origin's sentence for a member: from which segment and version, where the root went, and whether a newer version exists. */
export function originLines(origin: MemberOrigin, names: { segment: string; parent: string | undefined }, latest: number | undefined): string[] {
  const { segment } = origin.insertion;
  const lines = [`From segment ${names.segment}, version ${String(segment.version)}${origin.root ? (names.parent === undefined ? ", inserted at the top level" : `, inserted under ${names.parent}`) : ""}.`];
  if (latest !== undefined && latest > segment.version) {
    lines.push(`Version ${String(latest)} is available.`);
  }
  return lines;
}
