// What a proposal's items and mutations say to a reviewer (C14): each in a line of plain
// words, naming nodes, roles, and kinds by title where the graph holds them.
import { childrenOfClause, decisionOf, operatorOf, valuesOf, type Clause } from "../authoring/condition.ts";
import { ruleWords } from "../authoring/dates.ts";
import type { Graph, Mutation } from "../authoring/graph.ts";
import { DIFF_LABELS, removedChoices, type Conflict, type ResolutionKind, type ReviewItem } from "./model.ts";

/** Names by key over the graphs a review shows (before and after), falling back to the key. */
export interface Names {
  node(key: string): string;
  role(key: string): string;
  kind(key: string): string;
  entity(key: string): string;
}

/** Names from `graphs` (the later one wins) and the deployment's entities. */
export function namesOf(graphs: (Graph | null | undefined)[], entities: readonly { key: string; name: string }[]): Names {
  const nodes = new Map<string, string>();
  const roles = new Map<string, string>();
  const kinds = new Map<string, string>([["k_owner", "owner"]]);
  for (const graph of graphs) {
    for (const node of graph?.nodes ?? []) {
      nodes.set(node.key, node.title);
    }
    for (const role of graph?.roles ?? []) {
      roles.set(role.key, role.title ?? role.id);
    }
    for (const kind of graph?.participation_kinds ?? []) {
      kinds.set(kind.key, kind.title ?? kind.id);
    }
  }
  const people = new Map(entities.map((entity) => [entity.key, entity.name]));
  return {
    node: (key) => nodes.get(key) ?? key,
    role: (key) => roles.get(key) ?? key,
    kind: (key) => kinds.get(key) ?? key,
    entity: (key) => people.get(key) ?? key,
  };
}

const quoted = (text: string) => `"${text}"`;

/** A reason's first line, cut to fit a sentence. */
const briefly = (text: string) => {
  const [line = ""] = text.split("\n");
  return line.length > 80 ? `${line.slice(0, 80)}...` : line;
};

function valueWords(value: unknown): string {
  if (value === null || value === undefined) {
    return "nothing";
  }
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

/** A5: a condition as people read it. */
export function clauseWords(clause: Clause, names: Names): string {
  const operator = operatorOf(clause);
  const decision = decisionOf(clause);
  if (decision === undefined) {
    const parts = childrenOfClause(clause).map((child) => clauseWords(child, names));
    return operator === "not" ? `not (${parts.join("")})` : `${operator === "all" ? "all" : "any"} of (${parts.join("; ")})`;
  }
  const values = valuesOf(clause).map(String).join(" or ");
  const words: Record<string, string> = { equals: "is", not_equals: "is not", in: "is one of", contains: "contains", answered: "is answered" };
  return `${quoted(names.node(decision))} ${words[operator] ?? operator}${values === "" ? "" : ` ${values}`}`;
}

/** A field's value as people read it: a condition or a date rule in words, anything else as written. */
export function fieldValueWords(field: string, value: unknown, names: Names): string {
  if (value === null || value === undefined) {
    return "nothing";
  }
  if (field === "relevant_when") {
    return clauseWords(value as Clause, names);
  }
  if (field === "due_by" || field === "not_before") {
    return ruleWords(value, (source) => names.node(source));
  }
  return typeof value === "string" ? quoted(value) : valueWords(value);
}

/** A field's name as people read it. */
export function fieldName(field: string): string {
  return field === "relevant_when" ? "condition" : field.replaceAll("_", " ");
}

/** The one field a field value sets, and the value. */
function fieldOf(value: Record<string, unknown>): [string, unknown] {
  const [entry] = Object.entries(value);
  return entry ?? ["field", undefined];
}

/** C14: one mutation in words. */
export function mutationWords(mutation: Mutation, names: Names): string {
  const node = (key: string) => quoted(names.node(key));
  switch (mutation.op) {
    case "add_node": {
      const parent = mutation.node.parent;
      return `Add the ${mutation.node.kind} ${quoted(mutation.node.title)}${parent == null ? " at the top level" : ` under ${node(parent)}`}`;
    }
    case "set_node_field": {
      const [field, value] = fieldOf(mutation.value);
      return `Set the ${fieldName(field)} of ${node(mutation.node)} to ${fieldValueWords(field, value, names)}`;
    }
    case "replace_node":
      return `Replace ${node(mutation.node.key)} whole`;
    case "remove_node": {
      const beneath = (mutation.removal.descendants ?? []).length;
      return `Remove ${node(mutation.removal.node)}${beneath === 0 ? "" : ` and the ${String(beneath)} beneath it`}`;
    }
    case "add_edge":
      return `${node(mutation.edge.node)} requires ${node(mutation.edge.requires)}`;
    case "remove_edge":
      return `${node(mutation.edge.node)} no longer requires ${node(mutation.edge.requires)}`;
    case "answer":
      return `Answer ${node(mutation.decision)}: ${valueWords(Object.values(mutation.value)[0])}${mutation.rationale == null ? "" : `, because ${quoted(briefly(mutation.rationale))}`}`;
    case "set_pin":
      return `Pin ${node(mutation.node)} to ${mutation.date}`;
    case "shift_pin":
      return `Shift the pin of ${node(mutation.node)} by ${String(mutation.offset_days)} days`;
    case "clear_pin":
      return `Unpin ${node(mutation.node)}`;
    case "transition":
      return `${typeof mutation.transition === "string" ? mutation.transition : "skip"} ${node(mutation.node)}`;
    case "upgrade":
      return `Upgrade to version ${String(mutation.to)}, with every change the merge applies cleanly`;
    case "relink":
      return `Follow ${mutation.lineage.route} version ${String(mutation.lineage.version)}`;
    case "set_atomic":
      return mutation.atomic ? `Mark ${node(mutation.node)} atomic` : `${node(mutation.node)} needs breaking down`;
    case "set_participation":
      return `Set the ${names.kind(mutation.kind)} of ${node(mutation.node)}`;
    case "clear_participation":
      return `Clear the ${names.kind(mutation.kind)} of ${node(mutation.node)}`;
    case "add_role":
    case "edit_role":
      return `${mutation.op === "add_role" ? "Add" : "Change"} the role ${quoted(mutation.role.title ?? mutation.role.id)}`;
    case "remove_role":
      return `Remove the role ${quoted(names.role(mutation.role))}`;
    default:
      return `${mutation.op.replaceAll("_", " ")}${"node" in mutation && typeof mutation.node === "string" ? ` on ${node(mutation.node)}` : ""}`;
  }
}

/** The node a mutation is about, when it is about one. */
export function mutationNode(mutation: Mutation): string | undefined {
  switch (mutation.op) {
    case "add_node":
    case "replace_node":
      return mutation.node.key;
    case "remove_node":
      return mutation.removal.node;
    case "add_edge":
    case "remove_edge":
      return mutation.edge.node;
    case "answer":
      return mutation.decision;
    default:
      return "node" in mutation && typeof mutation.node === "string" ? mutation.node : undefined;
  }
}

/** B7: what a conflict is about, in words. */
export function conflictWords(conflict: Conflict, names: Names): string {
  const node = (key: string) => quoted(names.node(key));
  switch (conflict.about) {
    case "field": {
      const [field, ours] = fieldOf(conflict.journey);
      const [, theirs] = fieldOf(conflict.route);
      const dangling = conflict.dangling === true ? " (it names a node this journey removed)" : "";
      return `${node(conflict.node)}: its ${fieldName(field)} is ${fieldValueWords(field, ours, names)} here and ${fieldValueWords(field, theirs, names)} in the route${dangling}`;
    }
    case "edge":
      return `${node(conflict.edge.node)} requiring ${node(conflict.edge.requires)}: ${conflict.journey ? "added" : "removed"} here, ${conflict.route ? "kept" : "dropped"} by the route`;
    case "participation":
      return `${node(conflict.node)}: its ${names.kind(conflict.kind)} differs here and in the route`;
    case "resource":
      return `${node(conflict.node)}: a resource differs here and in the route${conflict.dangling === true ? " (the route's names a node this journey removed)" : ""}`;
    case "shape":
      return `${node(conflict.journey.key)}: a ${conflict.journey.kind} here, a ${conflict.route.kind} in the route${conflict.answered === true ? "; its answer would be cleared" : ""}`;
    case "answer":
      return `${node(conflict.decision)}: its answer names ${removedChoices(conflict.answer, conflict.choices).map(quoted).join(", ")}, which the route no longer offers`;
    case "role":
      return conflict.route == null
        ? `The role ${quoted(names.role(conflict.role))}: the route removed it, and this journey still uses it`
        : `The role ${quoted(names.role(conflict.role))}: changed here and in the route`;
    case "kind":
      return conflict.route == null
        ? `The ${quoted(names.kind(conflict.kind))} participations: the route removed the kind, and this journey still uses it`
        : `The participation kind ${quoted(names.kind(conflict.kind))}: changed here and in the route`;
    case "default_owner":
      return `The default owner: ${conflict.journey == null ? "none" : quoted(names.role(conflict.journey))} here, ${conflict.route == null ? "none" : quoted(names.role(conflict.route))} in the route`;
  }
}

/** B7: each resolution's words. */
export const RESOLUTION_WORDS: Record<ResolutionKind, string> = {
  keep_journey: "Keep this journey's",
  take_route: "Take the route's",
  clear_state: "Take the route's and clear what it invalidates",
  reopen: "Take the route's and reopen it",
  remove: "Remove it and what names it",
  remap_role: "Move its uses to another role",
  remap_kind: "Move its participations to another kind",
  map_choices: "Map each removed choice to one that remains",
};

/** C14: a review item's heading. */
export function itemHeading(item: ReviewItem, names: Names): string {
  switch (item.item) {
    case "conflict":
      return "Conflict";
    case "kept_local_edit":
      return "Kept local edit";
    case "orphan":
      return `Orphan: ${quoted(names.node(item.node))}, which the new version removed`;
    case "participation":
      return `Who stands in for ${quoted(names.entity(item.entity))} in the route`;
    case "exclusion":
      return quoted(names.node(item.node));
    case "cascade":
      return `Removal of ${quoted(names.node(item.removal.node))}`;
    case "violation":
      return "Violation";
  }
}

/** The words of a node's mark on the canvas, for the legend. */
export const LEGEND: string[] = Object.values(DIFF_LABELS);
