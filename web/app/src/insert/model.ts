// B13, C19: what inserting a route sends, put together from the engine's plan for the graph it
// goes into and the route version it copies (where each role and kind lands, what is left out,
// which roots are placed) and the person's choices. The rules are the engine's; this names
// things and wires the chosen nodes to the roots.
import type { Schema } from "@cairn/client";

import type { Graph, GraphNode, Kind, Mutation, Role } from "../authoring/graph.ts";
import { type DiffMark } from "../proposals/model.ts";

type Lineage = Schema<"Lineage">;

/** What a role or kind becomes: one the graph has, or a new one. */
export type Mapping = "add" | { existing: string };

/** What the person chose in the stepper's second and third steps; what they left alone takes its default. */
export interface Choices {
  /** The container the root goes under; none for the top level. */
  parent: string | undefined;
  /** The root's title; empty takes the route's own. */
  title: string;
  /** Graph nodes the root will require (Starts after). */
  after: readonly string[];
  /** Graph nodes that will require the root (Comes before). */
  before: readonly string[];
  /** Per route role and kind, what the person picked. */
  roles: Readonly<Record<string, Mapping>>;
  kinds: Readonly<Record<string, Mapping>>;
}

export const NO_CHOICES: Choices = { parent: undefined, title: "", after: [], before: [], roles: {}, kinds: {} };

/** One row of the People step: a route role or kind, what it can become, and what that means for the route's own filling decision. */
export interface PeopleRow {
  type: "role" | "kind";
  /** The route's key for it. */
  key: string;
  title: string;
  /** What it becomes, by default or as chosen. */
  mapping: Mapping;
  /** The graph's roles or kinds it may be mapped onto: those of the same cardinality (a mismatch is not offered). */
  options: readonly { key: string; title: string }[];
  /** Roles only: the graph already fills the chosen role, and the route's decisions that fill it are left out. */
  filled: { by: string | undefined; leftOut: readonly GraphNode[] } | undefined;
}

export interface Plan {
  mutation: Extract<Mutation, { op: "insert_route" }>;
  rows: readonly PeopleRow[];
  /** The root as it will be titled; none when the route has several roots. */
  root: { title: string } | undefined;
}

type Named = Role | Kind;

const titleOf = (item: Named) => item.title ?? item.id;
const titleIn = (items: readonly Named[], key: string) => {
  const found = items.find((item) => item.key === key);
  return found === undefined ? key : titleOf(found);
};
const mappingOf = (existing: string | undefined | null): Mapping => (existing == null ? "add" : { existing });

/** The People step's rows: the engine's plan (where each role and kind lands, and what that leaves out) with the titles to say it in. */
function peopleRows(host: Graph, route: Graph, plan: Schema<"InsertionPlan">): PeopleRow[] {
  const nodes = new Map((route.nodes ?? []).map((node) => [node.key, node] as const));
  const hostNodes = new Map((host.nodes ?? []).map((node) => [node.key, node] as const));
  const row = (type: PeopleRow["type"], held: readonly Named[], ours: readonly Named[]) => (each: Schema<"InsertionRow">): PeopleRow => ({
    type,
    key: each.key,
    title: titleIn(ours, each.key),
    mapping: mappingOf(each.existing),
    options: (each.options ?? []).map((key) => ({ key, title: titleIn(held, key) })),
    filled:
      each.filled == null
        ? undefined
        : {
            by: each.filled.by == null ? undefined : (hostNodes.get(each.filled.by)?.title ?? each.filled.by),
            leftOut: each.filled.left_out.flatMap((key) => nodes.get(key) ?? []),
          },
  });
  return [
    ...(plan.roles ?? []).map(row("role", host.roles ?? [], route.roles ?? [])),
    ...(plan.kinds ?? []).map(row("kind", host.participation_kinds ?? [], route.participation_kinds ?? [])),
  ];
}

/**
 * C19, B13: the `insert_route` mutation for `choices`, from the engine's plan: the roots'
 * wiring as edges (the root requires each Starts after node; each Comes before node requires
 * the root), every role and kind mapped explicitly, and the nodes the plan leaves out. `host`
 * is the graph it goes into.
 */
export function planInsertion(host: Graph, route: Graph, plan: Schema<"InsertionPlan">, lineage: Lineage, insertion: string, choices: Choices): Plan {
  const rows = peopleRows(host, route, plan);
  const roots = plan.roots ?? [];
  const only = roots.length === 1 ? (route.nodes ?? []).find((node) => node.key === roots[0]) : undefined;
  const named = only !== undefined && choices.title.trim() !== "";
  const rootTitle = named ? choices.title.trim() : (only?.title ?? "");
  const mappings = (type: PeopleRow["type"]) => Object.fromEntries(rows.filter((row) => row.type === type).map((row) => [row.key, row.mapping]));
  const mutation: Plan["mutation"] = {
    op: "insert_route",
    insertion,
    route: lineage,
    ...(choices.parent === undefined ? {} : { parent: choices.parent }),
    ...(named ? { root_title: rootTitle } : {}),
    roles: mappings("role"),
    kinds: mappings("kind"),
    omit: plan.omit ?? [],
    edges: roots.flatMap((root) => [
      ...choices.after.map((host) => ({ node: { route: root }, requires: { host } })),
      ...choices.before.map((host) => ({ node: { host }, requires: { route: root } })),
    ]),
  };
  return { mutation, rows, root: only === undefined ? undefined : { title: rootTitle } };
}

/** "⧉ Security review v2": the route and version, as a card's foot and a list row say it. */
export function originWords(name: string, version: number): string {
  return `⧉ ${name} v${String(version)}`;
}

/** C19: the nodes `after` adds that came from a route (named by it and its version), so review and the stepper's preview mark them the same way. */
export function insertionMembers(before: Graph | null | undefined, after: Graph | null | undefined, nameOf: (route: string) => string): Record<string, string> {
  const held = new Set((before?.nodes ?? []).map((node) => node.key));
  return Object.fromEntries(
    (after?.nodes ?? []).flatMap((node) => (node.from_route == null || held.has(node.key) ? [] : [[node.key, originWords(nameOf(node.from_route.route), node.from_route.version)] as const])),
  );
}

/** The marks of a diff with each added member saying which insertion it belongs to, on the tag's hover and in the list's foot. */
export function noteOrigins(marks: Record<string, DiffMark>, members: Record<string, string>): Record<string, DiffMark> {
  return Object.fromEntries(Object.entries(marks).map(([key, mark]) => [key, members[key] !== undefined && mark.kind === "added" ? { ...mark, note: members[key] } : mark]));
}

/** The roles `after` has that `before` does not: what an insertion adds besides nodes. */
export function addedRoles(before: Graph | null | undefined, after: Graph | null | undefined): Role[] {
  const held = new Set((before?.roles ?? []).map((role) => role.key));
  return (after?.roles ?? []).filter((role) => !held.has(role.key));
}
