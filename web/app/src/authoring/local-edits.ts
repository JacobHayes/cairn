// B4: a journey's departures from its route. Each edit to a route-copied field, edge,
// participation, or resource sets a per-field local-edit marker, so upgrades leave it alone;
// "reset to route" writes the route's value back and clears the marker in one patch (the
// write marks again, so the clear comes after it, as an upgrade's resolution does). Removing
// a route-copied node leaves a tombstone so an upgrade does not re-add it. A tombstoned key is
// retired and can never return (Invariants), so restoring one adds a journey-local copy of
// the route's node and its removed subtree under fresh keys, its references to what the
// journey still holds kept
// (decisions/2026-10-07-restoring-a-tombstone-adds-a-local-copy-under-fresh-keys.md).
import { FLAG_DEFAULTS, OWNER_KIND, type Schema } from "@cairn/client";

import { fieldWords } from "../status/words.ts";

import { childrenOfClause, decisionOf, operatorOf, type Clause } from "./condition.ts";
import { partsOf, ruleOf } from "./dates.ts";
import type { DateRule } from "./fields.ts";
import { childrenOf, descendantsOf, pathOf, titleIn, treeOf, type Graph, type GraphNode, type Mutation, type NodeField, type Tree } from "./graph.ts";
import { mintKey, uniqueId } from "./keys.ts";
import { kindNamed } from "../detail/sections.tsx";

export type LocalEdit = Schema<"LocalEdit">;

/** A marker as people read it: the field, the requirement, the participation, or the resource edited. */
export function editWords(edit: LocalEdit, tree: Tree): string {
  if (edit === "shape") {
    return "its kind or answer type";
  }
  if ("field" in edit) {
    return fieldWords(edit.field);
  }
  if ("requires" in edit) {
    return `its requirement on "${titleIn(tree, edit.requires)}"`;
  }
  if ("participation" in edit) {
    const kind = kindNamed(tree.graph.participation_kinds, edit.participation);
    return `its ${kind} participation`;
  }
  const title = [...tree.byKey.values()].flatMap((node) => node.resources ?? []).find((resource) => resource.key === edit.resource)?.title;
  return title == null ? "a resource" : `the resource "${title}"`;
}

/** The route node's value of `field`, as `set_node_field` writes it: a flag it leaves out reads as the engine's default (A1a). */
export function routeValue(route: GraphNode, field: NodeField): Schema<"NodeFieldValueResolved"> {
  const empty = field === "checklist" ? [] : null;
  const value = (route as Record<string, unknown>)[field] ?? (FLAG_DEFAULTS as Partial<Record<string, boolean>>)[field] ?? empty;
  return { [field]: value } as Schema<"NodeFieldValueResolved">;
}

/**
 * B4: the mutations that put `edit` of the journey's `node` back to the route's `route` and
 * clear its marker; none when the route's node is not there to reset to, or for a shape
 * change (the upgrade's review settles a kind or answer-type change, B7).
 */
export function resetMutations(node: GraphNode, route: GraphNode | undefined, edit: LocalEdit): Mutation[] | undefined {
  if (route === undefined || edit === "shape") {
    return undefined;
  }
  const clear: Mutation = { op: "mark_local_edit", node: node.key, edit, marked: false };
  if ("field" in edit) {
    return [{ op: "set_node_field", node: node.key, value: routeValue(route, edit.field) }, clear];
  }
  if ("requires" in edit) {
    const edge = { node: node.key, requires: edit.requires };
    const wanted = (route.requires ?? []).includes(edit.requires);
    const held = (node.requires ?? []).includes(edit.requires);
    const write: Mutation[] = wanted === held ? [] : [wanted ? { op: "add_edge", edge } : { op: "remove_edge", edge }];
    return [...write, clear];
  }
  if ("participation" in edit) {
    const kind = edit.participation;
    const source = route.participations?.[kind];
    const held = node.participations?.[kind] !== undefined;
    const write: Mutation[] = source !== undefined ? [{ op: "set_participation", node: node.key, kind, source }] : held ? [{ op: "clear_participation", node: node.key, kind }] : [];
    return [...write, clear];
  }
  const wanted = (route.resources ?? []).find((resource) => resource.key === edit.resource);
  const held = (node.resources ?? []).some((resource) => resource.key === edit.resource);
  const write: Mutation[] =
    wanted !== undefined
      ? [held ? { op: "edit_resource", node: node.key, resource: wanted } : { op: "add_resource", node: node.key, resource: wanted }]
      : held
        ? [{ op: "remove_resource", node: node.key, resource: edit.resource }]
        : [];
  return [...write, clear];
}

/** A tombstone a journey shows: the removed route node at the top of what was removed, and whether something holds its place now. */
export interface Tombstoned {
  node: GraphNode;
  path: string;
  /** The removed nodes beneath it. */
  beneath: number;
  /** A journey node already sits at its path (an earlier restore, or one added there). */
  occupied: boolean;
}

/** B4: the journey's tombstones as the tops of what was removed, read against the route version it follows. */
export function tombstonesOf(journey: Graph, version: Graph): Tombstoned[] {
  const route = treeOf(version);
  const held = treeOf(journey);
  const stones = new Set(journey.state?.tombstones ?? []);
  const paths = new Set([...held.byKey.keys()].map((key) => pathOf(held, key)));
  return [...stones].flatMap((key) => {
    const node = route.byKey.get(key);
    if (node === undefined || (node.parent != null && stones.has(node.parent))) {
      return [];
    }
    const path = pathOf(route, key);
    const beneath = descendantsOf(route, key).filter((each) => stones.has(each.key)).length;
    return [{ node, path, beneath, occupied: paths.has(path) }];
  });
}

/** A condition without the clauses naming any of `gone`; undefined when nothing is left. */
function withoutDecisions(clause: Clause, gone: ReadonlySet<string>): Clause | undefined {
  const decision = decisionOf(clause);
  if (decision !== undefined) {
    return gone.has(decision) ? undefined : clause;
  }
  const kept = childrenOfClause(clause).flatMap((child) => {
    const next = withoutDecisions(child, gone);
    return next === undefined ? [] : [next];
  });
  if (kept.length === 0) {
    return undefined;
  }
  switch (operatorOf(clause)) {
    case "not":
      return { not: kept[0] as Clause };
    case "any":
      return { any: kept };
    default:
      return { all: kept };
  }
}

/** A message draft's answer placeholders naming any of `gone` (by key, as stored), dropped. */
function withoutPlaceholders(template: string, gone: ReadonlySet<string>): string {
  return template.replace(/\{\{\s*answers\.([^}\s]+)\s*\}\}/g, (whole, answer: string) => (gone.has(answer) ? "" : whole));
}

/** A condition with every decision renamed by `rename`, or dropped where it gives none. */
function mapClause(clause: Clause, rename: (key: string) => string | undefined): Clause | undefined {
  const gone = new Set<string>();
  const collect = (each: Clause) => {
    const decision = decisionOf(each);
    if (decision !== undefined && rename(decision) === undefined) {
      gone.add(decision);
    }
    childrenOfClause(each).forEach(collect);
  };
  collect(clause);
  const kept = withoutDecisions(clause, gone);
  if (kept === undefined) {
    return undefined;
  }
  const text = JSON.stringify(kept, (name, value: unknown) =>
    (name === "decision" || name === "answered") && typeof value === "string" ? (rename(value) ?? value) : value,
  );
  return JSON.parse(text) as Clause;
}

/** A date rule with its sources renamed, or none when no source is left. */
function mapRule(rule: DateRule | null | undefined, rename: (key: string) => string | undefined): DateRule | undefined {
  if (rule == null) {
    return undefined;
  }
  const parts = partsOf(rule);
  const sources = parts.sources.flatMap((source) => (source === "journey.created_at" ? [source] : (rename(source) ?? [])));
  return sources.length === 0 ? undefined : ruleOf({ ...parts, sources });
}

/** One restored node: the route's node under its new key, with references to what the journey holds. */
function restoredNode(node: GraphNode, keys: ReadonlyMap<string, string>, journey: Tree, parent: string | undefined, id: string): GraphNode {
  const rename = (key: string) => keys.get(key) ?? (journey.byKey.has(key) ? key : undefined);
  const roles = new Set((journey.graph.roles ?? []).map((role) => role.key));
  const kinds = new Set<string>([OWNER_KIND.key, ...(journey.graph.participation_kinds ?? []).map((kind) => kind.key)]);
  const references: Record<string, unknown> = {
    parent,
    requires: (node.requires ?? []).flatMap((key) => rename(key) ?? []),
    relevant_when: node.relevant_when == null ? undefined : mapClause(node.relevant_when, rename),
    due_by: mapRule(node.due_by, rename),
    not_before: mapRule(node.not_before, rename),
    opens_at: node.opens_at === undefined ? undefined : rename(node.opens_at),
    closes_at: node.closes_at === undefined ? undefined : rename(node.closes_at),
    feeds_milestone: node.feeds_milestone === undefined ? undefined : rename(node.feeds_milestone),
    fills_role: node.fills_role !== undefined && roles.has(node.fills_role) ? node.fills_role : undefined,
    participations: Object.fromEntries(
      Object.entries(node.participations ?? {}).filter(([kind, source]) => kinds.has(kind) && (Array.isArray(source) || roles.has(source))),
    ),
    resources: (node.resources ?? []).map((resource) => restoredResource(resource, rename)),
  };
  const own = Object.entries(node).filter(([name]) => !(name in references));
  const set = Object.entries(references).filter(([, value]) => value !== undefined);
  return { ...(Object.fromEntries([...own, ...set]) as GraphNode), key: keys.get(node.key) ?? node.key, id };
}

function restoredResource(resource: Schema<"Resource">, rename: (key: string) => string | undefined): Schema<"Resource"> {
  const copy = { ...resource, key: mintKey("a_") };
  if (copy.message_draft === undefined) {
    return copy;
  }
  const answers = [...copy.message_draft.matchAll(/\{\{\s*answers\.([^}\s]+)\s*\}\}/g)].map((match) => match[1] ?? "");
  const gone = new Set(answers.filter((key) => rename(key) === undefined));
  const kept = withoutPlaceholders(copy.message_draft, gone);
  return { ...copy, message_draft: kept.replace(/\{\{\s*answers\.([^}\s]+)\s*\}\}/g, (whole, key: string) => `{{answers.${rename(key) ?? key}}}`) };
}

/** B4: restoring tombstoned `key`: a local copy of the route's node and its removed subtree, parents first. */
export function restoreMutations(journey: Graph, version: Graph, key: string): Mutation[] {
  const route = treeOf(version);
  const held = treeOf(journey);
  const stones = new Set(journey.state?.tombstones ?? []);
  const top = route.byKey.get(key);
  if (top === undefined) {
    return [];
  }
  const nodes = [top, ...descendantsOf(route, key).filter((node) => stones.has(node.key))];
  const keys = new Map(nodes.map((node) => [node.key, mintKey("n_")]));
  const parentOf = (node: GraphNode): string | undefined => {
    const parent = node.parent ?? undefined;
    return parent === undefined ? undefined : (keys.get(parent) ?? (held.byKey.has(parent) ? parent : undefined));
  };
  const taken = new Map<string, Set<string>>();
  return nodes.map((node): Mutation => {
    const parent = parentOf(node);
    const siblings = taken.get(parent ?? "") ?? new Set(childrenOf(held, parent).map((each) => each.id));
    const id = uniqueId(node.id, siblings);
    taken.set(parent ?? "", new Set([...siblings, id]));
    return { op: "add_node", node: restoredNode(node, keys, held, parent, id) };
  });
}
