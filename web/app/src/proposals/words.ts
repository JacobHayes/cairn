// What a proposal's items and mutations say to a reviewer (C14): each in a line of plain
// words, naming nodes, roles, and kinds by title where the graph holds them.
import { conditionNames, conditionWords, type Clause, type ConditionNames } from "../authoring/condition.ts";
import { ruleWords } from "../authoring/dates.ts";
import type { Graph, GraphNode, Mutation } from "../authoring/graph.ts";
import { answerWords, choiceNamed, choiceLabel } from "../status/answer.ts";
import { SNOOZE_NODE_TAIL } from "../status/snooze.ts";
import { dateWords } from "../status/when.ts";
import { fieldWords, upperFirst } from "../status/words.ts";
import { ENTITY_GONE, KIND_GONE, NODE_GONE, ROLE_GONE } from "../ui/gone.ts";
import { removedChoices, type Conflict, type ResolutionKind, type ReviewItem } from "./model.ts";

/** Names by key over the graphs a review shows (before and after), falling back to the key. */
export interface Names {
  node: (key: string) => string;
  role: (key: string) => string;
  kind: (key: string) => string;
  entity: (key: string) => string;
  /** What an answer to `decision` calls its choice `id`. */
  choice: (decision: string, id: string) => string;
  /** What a condition calls its decisions and the values it compares. */
  condition: ConditionNames;
  /** A date in words, in the year of today when it is known. */
  date: (date: string) => string;
}

/** Names from `graphs` (the later one wins) and the deployment's entities. */
export function namesOf(graphs: (Graph | null | undefined)[], entities: readonly { key: string; name: string }[], today?: string): Names {
  const nodes = new Map<string, string>();
  const decisions = new Map<string, GraphNode>();
  const roles = new Map<string, string>();
  const kinds = new Map<string, string>([["k_owner", "owner"]]);
  for (const graph of graphs) {
    for (const node of graph?.nodes ?? []) {
      nodes.set(node.key, node.title);
      decisions.set(node.key, node);
    }
    for (const role of graph?.roles ?? []) {
      roles.set(role.key, role.title ?? role.id);
    }
    for (const kind of graph?.participation_kinds ?? []) {
      kinds.set(kind.key, kind.title ?? kind.id);
    }
  }
  const people = new Map(entities.map((entity) => [entity.key, entity.name]));
  const entity = (key: string) => people.get(key) ?? ENTITY_GONE;
  return {
    node: (key) => nodes.get(key) ?? NODE_GONE,
    role: (key) => roles.get(key) ?? ROLE_GONE,
    kind: (key) => kinds.get(key) ?? KIND_GONE,
    entity,
    choice: (decision, id) => choiceNamed(decisions.get(decision)?.choices, id),
    condition: conditionNames((decision) => decisions.get(decision), entity),
    date: (date) => dateWords(date, today),
  };
}

const quoted = (text: string) => `"${text}"`;

/** A reason's first line, cut to fit a sentence. */
const briefly = (text: string) => {
  const [line = ""] = text.split("\n");
  return line.length > 80 ? `${line.slice(0, 80)}...` : line;
};

/** A field's value as people read it: a condition, a date rule, a node, a role or the choices in words, anything else as written. */
export function fieldValueWords(field: string, value: unknown, names: Names): string {
  if (value === null || value === undefined) {
    return "nothing";
  }
  switch (field) {
    case "relevant_when":
      return conditionWords(value as Clause, names.condition);
    case "due_by":
    case "not_before":
      return ruleWords(value, (source) => names.node(source));
    case "parent":
    case "opens_at":
    case "closes_at":
    case "feeds_milestone":
      return typeof value === "string" ? quoted(names.node(value)) : JSON.stringify(value);
    case "fills_role":
      return typeof value === "string" ? quoted(names.role(value)) : JSON.stringify(value);
    case "choices":
      return Array.isArray(value) ? value.map((choice: Parameters<typeof choiceLabel>[0]) => quoted(choiceLabel(choice))).join(", ") : JSON.stringify(value);
    default:
      return typeof value === "boolean" ? (value ? "yes" : "no") : typeof value === "string" ? quoted(value) : typeof value === "number" ? String(value) : JSON.stringify(value);
  }
}

/** The one field a field value sets, and the value. */
function fieldOf(value: Record<string, unknown>): [string, unknown] {
  const [entry] = Object.entries(value);
  return entry ?? ["field", undefined];
}

/** A node's field set to a value, in words. */
function fieldSetWords(node: string, set: Record<string, unknown>, names: Names): string {
  const [field, value] = fieldOf(set);
  if (field === "closes" && value === false) {
    return `Let the stage ${node} run past its close`;
  }
  if (field === "estimate" && typeof value === "number") {
    return `Set the estimate of ${node} to ${String(value)} days`;
  }
  return `Set the ${fieldWords(field)} of ${node} to ${fieldValueWords(field, value, names)}`;
}

/** C14: one mutation in words. */
export function mutationWords(mutation: Mutation, names: Names): string {
  const node = (key: string) => quoted(names.node(key));
  switch (mutation.op) {
    case "add_node": {
      const parent = mutation.node.parent;
      return `Add the ${mutation.node.kind} ${quoted(mutation.node.title)}${parent == null ? " at the top level" : ` under ${node(parent)}`}`;
    }
    case "set_node_field":
      return fieldSetWords(node(mutation.node), mutation.value, names);
    case "replace_node":
      return `Replace ${node(mutation.node.key)} whole`;
    case "remove_node": {
      const beneath = (mutation.removal.descendants ?? []).length;
      return `Remove ${node(mutation.removal.node)}${beneath === 0 ? "" : ` and the ${String(beneath)} beneath it`}`;
    }
    case "add_edge":
      return `${node(mutation.edge.node)} requires ${node(mutation.edge.requires)}`;
    case "remove_edge":
      return `Drop the requirement of ${node(mutation.edge.node)} on ${node(mutation.edge.requires)}`;
    case "answer": {
      const value = answerWords(mutation.value, { choice: (id) => names.choice(mutation.decision, id), entity: (key) => names.entity(key), date: names.date });
      return `Answer ${node(mutation.decision)}: ${value}${mutation.rationale == null ? "" : `, because ${quoted(briefly(mutation.rationale))}`}`;
    }
    case "set_pin":
      return `Pin ${node(mutation.node)} to ${names.date(mutation.date)}`;
    case "shift_pin": {
      const days = mutation.offset_days;
      return `Move the pin on ${node(mutation.node)} ${String(Math.abs(days))} days ${days < 0 ? "earlier" : "later"}`;
    }
    case "clear_pin":
      return `Unpin ${node(mutation.node)}`;
    case "transition": {
      const move = typeof mutation.transition === "string" ? mutation.transition : "skip";
      return `${move === "reach" ? "Mark reached" : upperFirst(move)} ${node(mutation.node)}`;
    }
    case "snooze":
      return `Snooze ${node(mutation.node)} until ${"date" in mutation.until ? names.date(mutation.until.date) : `${node(mutation.until.node)}${SNOOZE_NODE_TAIL}`}`;
    case "unsnooze":
      return `Wake ${node(mutation.node)}`;
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
    case "insert_segment": {
      const parent = mutation.parent;
      const left = (mutation.omit ?? []).length;
      return `Insert the segment ${mutation.segment.route} version ${String(mutation.segment.version)}${parent == null ? " at the top level" : ` under ${node(parent)}`}${left === 0 ? "" : `, leaving ${String(left)} ${left === 1 ? "step" : "steps"} out`}`;
    }
    default:
      return `${mutation.op.replaceAll("_", " ")}${"node" in mutation && typeof mutation.node === "string" ? ` on ${node(mutation.node)}` : ""}`;
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
      return `${node(conflict.node)}: its ${fieldWords(field)} differs. Here: ${fieldValueWords(field, ours, names)}. In the route: ${fieldValueWords(field, theirs, names)}${dangling}`;
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

