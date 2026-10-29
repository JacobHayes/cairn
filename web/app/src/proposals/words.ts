// What a proposal's items and mutations say to a reviewer (C14): each in a line of plain
// words, naming nodes, roles, and kinds by title where the graph holds them.
import type { Schema } from "@cairn/client";

import { conditionNames, conditionWords, type Clause, type ConditionNames } from "../authoring/condition.ts";
import { ruleWords } from "../authoring/dates.ts";
import type { Graph, GraphNode, Mutation } from "../authoring/graph.ts";
import { answerWords, choiceNamed, choiceLabel } from "../status/answer.ts";
import { SNOOZE_NODE_TAIL } from "../status/snooze.ts";
import { dateWords } from "../status/when.ts";
import { fieldWords, upperFirst } from "../status/words.ts";
import { ITEM_GONE, nodeTitle } from "../ui/gone.ts";
import type { ReviewItem } from "./model.ts";
import { plural } from "../ui/words.ts";
import { moveOf } from "../detail/model.ts";
import { identityNamed, kindNamed, roleNamed, type Slot } from "../detail/sections.tsx";
import type { Deployment } from "../data/host.ts";

/** Names by key over the graphs a review shows (before and after), falling back to words for what is gone. */
export interface Names {
  node: (key: string) => string;
  /** A checklist item of node `node` by its title. */
  item: (node: string, id: string) => string;
  role: (key: string) => string;
  kind: (key: string) => string;
  identity: (key: string) => string;
  /** What an answer to `decision` calls its choice `id`. */
  choice: (decision: string, id: string) => string;
  /** What a condition calls its decisions and the values it compares. */
  condition: ConditionNames;
  /** A date in words, in the year of today when it is known. */
  date: (date: string) => string;
}

/** Names from `graphs` (the later one wins) and the deployment's identities, through its merges. */
export function namesOf(graphs: (Graph | null | undefined)[], deployment: Deployment, today?: string): Names {
  const nodes = new Map<string, GraphNode>();
  const roles = new Map<string, Slot>();
  const kinds = new Map<string, Slot>();
  for (const graph of graphs) {
    for (const node of graph?.nodes ?? []) {
      nodes.set(node.key, node);
    }
    for (const role of graph?.roles ?? []) {
      roles.set(role.key, role);
    }
    for (const kind of graph?.participation_kinds ?? []) {
      kinds.set(kind.key, kind);
    }
  }
  const identity = (key: string) => identityNamed(deployment, key);
  return {
    node: (key) => nodeTitle(nodes.get(key)?.title),
    item: (node, id) => nodes.get(node)?.checklist?.find((item) => item.id === id)?.title ?? ITEM_GONE,
    role: (key) => roleNamed([...roles.values()], key),
    kind: (key) => kindNamed([...kinds.values()], key),
    identity,
    choice: (decision, id) => choiceNamed(nodes.get(decision)?.choices, id),
    condition: conditionNames((decision) => nodes.get(decision), identity),
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
    case "checklist":
      return Array.isArray(value) ? (value.length === 0 ? "no items" : value.map((item: { title: string }) => quoted(item.title)).join(", ")) : JSON.stringify(value);
    default:
      return typeof value === "boolean" ? (value ? "yes" : "no") : typeof value === "string" ? quoted(value) : typeof value === "number" ? String(value) : JSON.stringify(value);
  }
}

/** What a journey's state holds of one node under `field` (nodes, answers, rationales, overrides, pins, snoozes), in a person's words: names and dates, never a key or the record's own shape. */
export function stateValueWords(field: string, value: unknown, node: string, names: Names): string {
  if (value === null || value === undefined) {
    return "nothing";
  }
  const record = value as Record<string, unknown>;
  switch (field) {
    case "nodes": {
      const own = record as { state?: string; started_on?: string; finished_on?: string; reason?: string; atomic?: boolean };
      const parts = [
        own.state === undefined ? undefined : own.state.replaceAll("_", " "),
        own.started_on === undefined ? undefined : `started ${names.date(own.started_on)}`,
        own.finished_on === undefined ? undefined : `finished ${names.date(own.finished_on)}`,
        own.reason === undefined ? undefined : `because ${quoted(briefly(own.reason))}`,
        own.atomic === true ? "atomic" : undefined,
      ];
      return parts.filter((part) => part !== undefined).join(", ");
    }
    case "answers":
      return answerWords(value as Parameters<typeof answerWords>[0], { choice: (id) => names.choice(node, id), identity: (key) => names.identity(key), date: names.date });
    case "rationales":
      return typeof value === "string" ? quoted(briefly(value)) : "a reason";
    case "pins":
      return typeof value === "string" ? names.date(value) : "a date";
    case "ticks":
      return Array.isArray(value) && value.length > 0 ? value.map((item) => quoted(names.item(node, String(item)))).join(", ") : "nothing ticked";
    case "snoozes":
      return "date" in record && typeof record["date"] === "string" ? `until ${names.date(record["date"])}` : typeof record["node"] === "string" ? `until ${quoted(names.node(record["node"]))}${SNOOZE_NODE_TAIL}` : "set aside";
    case "overrides":
      return [record["force_include"] == null ? undefined : "forced in", record["keep"] == null ? undefined : "kept", record["bypass"] == null ? undefined : "its guard bypassed"].filter((part) => part !== undefined).join(", ");
    default:
      return "changed";
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
      const value = answerWords(mutation.value, { choice: (id) => names.choice(mutation.decision, id), identity: (key) => names.identity(key), date: names.date });
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
      const move = moveOf(mutation.transition);
      return move === "mark_done" ? `Mark ${node(mutation.node)} done` : `${upperFirst(move)} ${node(mutation.node)}`;
    }
    case "snooze":
      return `Snooze ${node(mutation.node)} until ${"date" in mutation.until ? names.date(mutation.until.date) : `${node(mutation.until.node)}${SNOOZE_NODE_TAIL}`}`;
    case "unsnooze":
      return `Wake ${node(mutation.node)}`;
    case "set_ticked":
      return `${mutation.ticked ? "Tick" : "Untick"} ${quoted(names.item(mutation.node, mutation.item))} on ${node(mutation.node)}`;
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
      return `Insert the segment ${mutation.segment.route} version ${String(mutation.segment.version)}${parent == null ? " at the top level" : ` under ${node(parent)}`}${left === 0 ? "" : `, leaving ${plural(left, "step")} out`}`;
    }
    default:
      return `${mutation.op.replaceAll("_", " ")}${"node" in mutation && typeof mutation.node === "string" ? ` on ${node(mutation.node)}` : ""}`;
  }
}

/** C14: a review item's heading. */
export function itemHeading(item: ReviewItem, names: Names): string {
  switch (item.item) {
    case "participation":
      return `Who stands in for ${quoted(names.identity(item.identity))} in the route`;
    case "exclusion":
      return quoted(names.node(item.node));
    case "cascade":
      return `Removal of ${quoted(names.node(item.removal.node))}`;
    case "violation":
      return "Violation";
  }
}


type Outline = Schema<"ProposalOutline">;
type OutlineFact = Schema<"OutlineFact">;

/** One header or lifecycle fact of a proposal's outline, as a phrase that follows "It". */
function factWords(fact: OutlineFact): string {
  switch (fact.fact) {
    case "creates_journey":
      return "creates the journey";
    case "edits_journey":
      return "edits the journey";
    case "sets_journey_status":
      return `sets the journey to ${fact.status}`;
    case "creates_route":
      return "creates the route";
    case "edits_route":
      return "edits the route";
    case "retires_route":
      return "retires the route";
    case "restores_route":
      return "restores the route";
    case "opens_draft":
      return "opens a draft";
    case "discards_draft":
      return "discards the draft";
    case "publishes_draft":
      return "publishes the draft";
    case "inserts_segment":
      return "inserts a segment";
  }
}

const counted = (count: number, one: string, many: string) => `${String(count)} ${count === 1 ? one : many}`;

/**
 * C14: what a proposal changes, in one sentence, from the outline the engine worked out of its
 * mutations (the listing's and the preview's are the same value): its header and lifecycle facts
 * first, then the nodes, requirements and identities it names. "Creates the route and adds 1 node."
 */
export function outlineWords(outline: Outline): string {
  const phrases = [
    ...(outline.facts ?? []).map(factWords),
    ...(outline.nodes_added ? [`adds ${counted(outline.nodes_added, "node", "nodes")}`] : []),
    ...(outline.nodes_changed ? [`changes ${counted(outline.nodes_changed, "node", "nodes")}`] : []),
    ...(outline.nodes_removed ? [`removes ${counted(outline.nodes_removed, "node", "nodes")}`] : []),
    ...(outline.requirements_added ? [`adds ${counted(outline.requirements_added, "requirement", "requirements")}`] : []),
    ...(outline.requirements_removed ? [`removes ${counted(outline.requirements_removed, "requirement", "requirements")}`] : []),
    ...(outline.identities_created ? [`creates ${counted(outline.identities_created, "identity", "identities")}`] : []),
    ...(outline.identities_edited ? [`edits ${counted(outline.identities_edited, "identity", "identities")}`] : []),
    ...(outline.identities_merged ? [`merges ${counted(outline.identities_merged, "identity", "identities")}`] : []),
    ...(outline.identities_deleted ? [`deletes ${counted(outline.identities_deleted, "identity", "identities")}`] : []),
    ...(outline.journeys_deleted ? [`deletes ${counted(outline.journeys_deleted, "journey", "journeys")}`] : []),
    ...(outline.routes_deleted ? [`deletes ${counted(outline.routes_deleted, "route", "routes")}`] : []),
    ...(outline.other ? ["adjusts roles, kinds or defaults"] : []),
  ];
  if (phrases.length === 0) {
    return "Changes nothing.";
  }
  const [last, ...rest] = [...phrases].reverse();
  const joined = rest.length === 0 ? (last ?? "") : `${rest.reverse().join(", ")} and ${last ?? ""}`;
  return `${joined.charAt(0).toUpperCase()}${joined.slice(1)}.`;
}
