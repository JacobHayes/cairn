// A18: what removing a node takes with it, and what else must change in the same patch. The
// removal names everything it removes as the author sees it now (the subtree, every explicit
// edge into or out of it, and what hangs on those nodes: participations, resources, and in a
// journey notes and links), so the engine can refuse it if anything was added since rather
// than widen it. Every reference that would dangle (a condition's clause, a date rule's
// source, a stage bound, `feeds_milestone`, a snooze on the node, a message draft's answer
// placeholder) is rewritten or removed by a mutation sent first in the same patch. Removing a
// role or a kind does the same for what names it (participations, `fills_role`, the default
// owner, a draft's role placeholder).
//
// Cost: one walk of the subtree and one pass over every node, O(nodes + edges + resources).
import type { Schema } from "@cairn/client";

import { childrenOfClause, decisionOf, operatorOf, type Clause } from "./condition.ts";
import { partsOf, ruleOf } from "./dates.ts";
import type { DateRule } from "./fields.ts";
import { descendantsOf, titleIn, type GraphNode, type Mutation, type Resource, type Tree } from "./graph.ts";

export type Removal = Schema<"Removal">;

/** What a reference that would dangle is. */
export type DanglingKind = "condition" | "due_by" | "not_before" | "opens_at" | "closes_at" | "feeds_milestone" | "fills_role" | "participation" | "default_owner" | "snooze" | "message_draft";

/** A reference outside what is removed that names it, and the mutation that rewrites or removes it. */
export interface Dangling {
  /** The node holding the reference; none for the graph's default owner. */
  node: string | undefined;
  what: DanglingKind;
  /** What it does about it, in words. */
  fix: string;
  mutation: Mutation;
}

/** A removal and its cascade: what to show the author and what to send, in order. */
export interface RemovalPlan {
  removal: Removal;
  /** The nodes removed, the removed one first. */
  nodes: GraphNode[];
  dangling: Dangling[];
}

/** A condition without the clauses naming any of `gone`; undefined when nothing is left. */
export function withoutDecisions(clause: Clause, gone: ReadonlySet<string>): Clause | undefined {
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

/** A date rule without the sources in `gone`; null when it has none left. */
export function withoutSources(rule: DateRule, gone: ReadonlySet<string>): DateRule | null {
  const parts = partsOf(rule);
  const sources = parts.sources.filter((source) => !gone.has(source));
  return sources.length === 0 ? null : ruleOf({ ...parts, sources });
}

/** A message draft's placeholders naming any of `gone` (by key, as stored), dropped. */
export function withoutPlaceholders(template: string, gone: ReadonlySet<string>): string {
  return template.replace(/\{\{\s*(answers\.([^}\s]+)|roles\.([^}.\s]+)\.name)\s*\}\}/g, (whole, _all, answer: string | undefined, role: string | undefined) =>
    gone.has(answer ?? role ?? "") ? "" : whole,
  );
}

/** Whether a message draft names any of `gone`. */
function draftNames(resource: Resource, gone: ReadonlySet<string>): boolean {
  return resource.message_draft !== undefined && withoutPlaceholders(resource.message_draft, gone) !== resource.message_draft;
}

const field = (node: string, value: Schema<"NodeFieldValueResolved">): Mutation => ({ op: "set_node_field", node, value });

/** The references on `node` (which stays) that name any of `gone`, each with its fix. */
function danglingOn(node: GraphNode, gone: ReadonlySet<string>, tree: Tree): Dangling[] {
  const found: Dangling[] = [];
  const names = (keys: Iterable<string>) => [...keys].filter((key) => gone.has(key)).map((key) => `"${titleIn(tree, key)}"`).join(", ");
  if (node.relevant_when != null) {
    const kept = withoutDecisions(node.relevant_when, gone);
    if (JSON.stringify(kept) !== JSON.stringify(node.relevant_when)) {
      const fix = kept === undefined ? "clear its condition" : "drop the clauses that name it from its condition";
      found.push({ node: node.key, what: "condition", fix, mutation: field(node.key, { relevant_when: kept ?? null }) });
    }
  }
  for (const which of ["due_by", "not_before"] as const) {
    const rule = node[which];
    if (rule != null && partsOf(rule).sources.some((source) => gone.has(source))) {
      const kept = withoutSources(rule, gone);
      const value = which === "due_by" ? { due_by: kept } : { not_before: kept };
      found.push({ node: node.key, what: which, fix: kept === null ? `clear its ${which} rule` : `measure its ${which} rule without ${names(partsOf(rule).sources)}`, mutation: field(node.key, value) });
    }
  }
  for (const which of ["opens_at", "closes_at"] as const) {
    const bound = node[which];
    if (bound !== undefined && gone.has(bound)) {
      const value = which === "opens_at" ? { opens_at: null } : { closes_at: null };
      found.push({ node: node.key, what: which, fix: `clear its stage bound ${which}`, mutation: field(node.key, value) });
    }
  }
  if (node.feeds_milestone !== undefined && gone.has(node.feeds_milestone)) {
    found.push({ node: node.key, what: "feeds_milestone", fix: "stop it pinning the milestone", mutation: field(node.key, { feeds_milestone: null }) });
  }
  for (const resource of node.resources ?? []) {
    if (resource.message_draft !== undefined && draftNames(resource, gone)) {
      const next = { ...resource, message_draft: withoutPlaceholders(resource.message_draft, gone) };
      found.push({ node: node.key, what: "message_draft", fix: `drop the placeholders that name it from "${resource.title ?? "a message draft"}"`, mutation: { op: "edit_resource", node: node.key, resource: next } });
    }
  }
  return found;
}

/** A18: removing `key` from the graph `tree` reads (a journey's carries its state), with its cascade. */
export function planRemoval(tree: Tree, key: string): RemovalPlan | undefined {
  const root = tree.byKey.get(key);
  if (root === undefined) {
    return undefined;
  }
  const nodes = [root, ...descendantsOf(tree, key)];
  const gone = new Set(nodes.map((node) => node.key));
  const state = tree.graph.state;
  const removal: Removal = {
    node: key,
    descendants: nodes.slice(1).map((node) => node.key),
    edges: [...tree.byKey.values()].flatMap((node) =>
      (node.requires ?? []).filter((requires) => gone.has(node.key) || gone.has(requires)).map((requires) => ({ node: node.key, requires })),
    ),
    participations: nodes.flatMap((node) => Object.keys(node.participations ?? {}).map((kind) => ({ node: node.key, kind }))),
    resources: nodes.flatMap((node) => (node.resources ?? []).map((resource) => resource.key)),
    annotations: (state?.annotations ?? []).filter((annotation) => annotation.body.node != null && gone.has(annotation.body.node)).map((annotation) => annotation.body.key),
  };
  const staying = [...tree.byKey.values()].filter((node) => !gone.has(node.key));
  const dangling = staying.flatMap((node) => danglingOn(node, gone, tree));
  for (const [snoozed, target] of Object.entries(state?.snoozes ?? {})) {
    if (!gone.has(snoozed) && "node" in target && gone.has(target.node)) {
      dangling.push({ node: snoozed, what: "snooze", fix: "unsnooze it", mutation: { op: "unsnooze", node: snoozed } });
    }
  }
  return { removal, nodes, dangling };
}

/** What a removal plan sends, in order: the rewrites first, on nodes that stay, then the removal. */
export function removalMutations(plan: RemovalPlan): Mutation[] {
  return [...plan.dangling.map((each) => each.mutation), { op: "remove_node", removal: plan.removal }];
}

/** A18: removing role `role`, with what names it rewritten first. */
export function planRoleRemoval(tree: Tree, role: string): Dangling[] {
  const found: Dangling[] = [];
  const gone = new Set([role]);
  for (const node of tree.byKey.values()) {
    for (const [kind, source] of Object.entries(node.participations ?? {})) {
      if (source === role) {
        found.push({ node: node.key, what: "participation", fix: `clear its ${kind} participation`, mutation: { op: "clear_participation", node: node.key, kind } });
      }
    }
    if (node.fills_role === role) {
      found.push({ node: node.key, what: "fills_role", fix: "stop it filling the role", mutation: field(node.key, { fills_role: null }) });
    }
    for (const resource of node.resources ?? []) {
      if (resource.message_draft !== undefined && draftNames(resource, gone)) {
        const next = { ...resource, message_draft: withoutPlaceholders(resource.message_draft, gone) };
        found.push({ node: node.key, what: "message_draft", fix: `drop the role's placeholders from "${resource.title ?? "a message draft"}"`, mutation: { op: "edit_resource", node: node.key, resource: next } });
      }
    }
  }
  if (tree.graph.default_owner === role) {
    found.push({ node: undefined, what: "default_owner", fix: "leave the graph with no default owner", mutation: { op: "set_default_owner", role: null } });
  }
  return found;
}

/** A18: removing participation kind `kind`, with every participation of it cleared first. */
export function planKindRemoval(tree: Tree, kind: string): Dangling[] {
  return [...tree.byKey.values()].flatMap((node) =>
    Object.keys(node.participations ?? {}).includes(kind)
      ? [{ node: node.key, what: "participation" as const, fix: "clear its participation of this kind", mutation: { op: "clear_participation", node: node.key, kind } }]
      : [],
  );
}
