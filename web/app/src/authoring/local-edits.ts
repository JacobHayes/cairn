// B4: a journey's departures from its route. Each edit to a route-copied field, edge,
// participation, or resource sets a per-field local-edit marker, so upgrades leave it alone;
// "reset to route" writes the route's value back and clears the marker in one patch (the
// write marks again, so the clear comes after it, as an upgrade's resolution does). Removing
// a route-copied node leaves a tombstone so an upgrade does not re-add it. A tombstoned key is
// retired and can never return (Invariants), so restoring one adds a journey-local copy of
// the route's node and its removed subtree under fresh keys, its references to what the
// journey still holds kept
// (decisions/2026-10-07-restoring-a-tombstone-adds-a-local-copy-under-fresh-keys.md).
import { FLAG_DEFAULTS, type Schema } from "@cairn/client";

import { fieldWords } from "../status/words.ts";

import { descendantsOf, pathOf, titleIn, treeOf, type Graph, type GraphNode, type Mutation, type NodeField, type Tree } from "./graph.ts";
import type { Deriver } from "../data/host.ts";
import { mintKey } from "./keys.ts";
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

/** B4: restoring tombstoned `key`: a local copy of the route's node and its removed subtree, parents first, as the engine builds it; none when it cannot be restored whole. */
export async function restoreMutations(deriver: Deriver, journey: Graph, version: Graph, key: string): Promise<Mutation[] | undefined> {
  // Fresh keys are built by the engine from this random body.
  return (await deriver.restoreNodes(journey, version, key, mintKey("n_").slice("n_".length)));
}
