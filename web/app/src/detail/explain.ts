// The words behind node detail's explanations (C8, F7): each derived value's explanation
// inputs from the local derive, said in terms of the journey's nodes, and, for a date bound,
// the pin or rule to edit to change it. Pure functions of the derived document, so every
// screen that explains a value (the trace, the timeline) says it the same way.
import type { Schema } from "@cairn/client";

import { isLeaf } from "../authoring/graph.ts";
import { feedingDecision, nodeOf, titleOf, type Ready } from "./model.ts";
import { dateWords, originWord } from "../status/when.ts";
import { fieldWords, upperFirst } from "../status/words.ts";

import { roleTitle } from "./sections.tsx";

export type Bound = Schema<"Bound">;
export type Chain = Schema<"Chain">;
export type Constraint = Schema<"Constraint">;
export type FixedDate = Schema<"FixedDate">;
export type DependencyVia = Schema<"DependencyVia">;

/** An instant as the server writes it (`n_x.start`, `n_x.finish`, `n_x.answer`, `created`), read. */
export type Instant = { created: true } | { node: string; point: "start" | "finish" } | { answer: string };

/** Where a constraint comes from, read from the server's `kind:node:node` text. */
export type ConstraintSource =
  | { due_by: { node: string } }
  | { not_before: { node: string } }
  | { estimate: { node: string } }
  | { containment: { parent: string; child: string } }
  | { dependency: { node: string; requires: string; via: DependencyVia } };

/** Reads an instant the server wrote. */
export function parseInstant(text: Schema<"Instant">): Instant {
  if (text === "created") {
    return { created: true };
  }
  const at = text.lastIndexOf(".");
  const node = text.slice(0, at);
  const point = text.slice(at + 1);
  if (at < 0 || (point !== "start" && point !== "finish" && point !== "answer")) {
    throw new Error(`not an instant: ${text}`);
  }
  return point === "answer" ? { answer: node } : { node, point };
}

/** Reads a constraint source the server wrote. */
export function parseSource(text: Schema<"ConstraintSource">): ConstraintSource {
  const [kind = "", a = "", b = "", ...via] = text.split(":");
  switch (kind) {
    case "due_by":
    case "not_before":
    case "estimate":
      return { [kind]: { node: a } } as ConstraintSource;
    case "containment":
      return { containment: { parent: a, child: b } };
    case "dependency":
      return { dependency: { node: a, requires: b, via: parseVia(via) } };
    default:
      throw new Error(`not a constraint source: ${text}`);
  }
}

function parseVia([kind = "", key = ""]: string[]): DependencyVia {
  switch (kind) {
    case "explicit":
    case "containment":
      return kind;
    case "inherited":
      return { inherited: { ancestor: key } };
    case "condition":
      return { condition: { condition_on: key } };
    default:
      throw new Error(`not a dependency origin: ${kind}`);
  }
}

/**
 * Names nodes by title: the panel's one way of saying which node it means. Given a point, it
 * names that instant: a start or finish of work, or a decision's or milestone's one instant.
 */
export type Namer = (key: string, point?: "start" | "finish") => string;

export function namer(view: Ready): Namer {
  return (key, point) => {
    const kind = nodeOf(view, key)?.kind;
    const single = kind !== undefined && isLeaf(kind);
    return point === undefined || single ? titleOf(view, key) : `the ${point} of ${titleOf(view, key)}`;
  };
}

/** An instant: a node's start or finish, a date answer, or the journey's creation. */
export function instantText(text: Schema<"Instant">, name: Namer): string {
  const instant = parseInstant(text);
  if ("created" in instant) {
    return "the journey's creation";
  }
  if ("answer" in instant) {
    return `the answer to ${name(instant.answer)}`;
  }
  return name(instant.node, instant.point);
}

/** How a dependency arose (Gating: implicit edges name their source). */
export function viaText(via: DependencyVia, name: Namer): string {
  if (via === "explicit") {
    return "an explicit requirement";
  }
  if (via === "containment") {
    return "a child it contains";
  }
  if ("inherited" in via) {
    return `a requirement inherited from ${name(via.inherited.ancestor)}`;
  }
  return `the condition on ${name(via.condition.condition_on)}`;
}

/** Where a constraint comes from (F2), in words. */
export function sourceText(text: Schema<"ConstraintSource">, name: Namer): string {
  return parsedSourceText(parseSource(text), name);
}

/** [`sourceText`] for a source already read. */
export function parsedSourceText(source: ConstraintSource, name: Namer): string {
  if ("due_by" in source) {
    return `the due date rule of ${name(source.due_by.node)}`;
  }
  if ("not_before" in source) {
    return `the earliest start rule of ${name(source.not_before.node)}`;
  }
  if ("dependency" in source) {
    const { node, requires, via } = source.dependency;
    return `${name(node)} requires ${name(requires)}, through ${viaText(via, name)}`;
  }
  if ("estimate" in source) {
    return `the estimate of ${name(source.estimate.node)}`;
  }
  return `${name(source.containment.parent)} contains ${name(source.containment.child)}`;
}

/** One constraint as the chain shows it: `after` at least `offset` days after `before`. */
export function constraintText(constraint: Constraint, name: Namer): string {
  const days = constraint.days;
  const count = (n: number) => `${String(n)} ${n === 1 ? "day" : "days"}`;
  const offset = days === 0 ? "" : days > 0 ? ` + ${count(days)}` : ` - ${count(-days)}`;
  return `${upperFirst(instantText(constraint.after, name))} no earlier than ${instantText(constraint.before, name)}${offset}`;
}

/** What fixes a date on a chain, in words. */
export function fixedText(fixed: FixedDate, name: Namer, today?: string): string {
  const by = { pin: originWord("pin"), actual: originWord("actual"), answer: "answered", today: "today" }[fixed.fixed_by];
  return `${upperFirst(instantText(fixed.instant, name))}: ${dateWords(fixed.date, today)} (${by})`;
}

/** F7: a pin or rule whose edit changes a bound. Actuals and today are facts, not edits. */
export type EditTarget =
  | { edit: "pin"; node: string }
  | { edit: "answer"; decision: string }
  | { edit: "due_by" | "not_before" | "estimate"; node: string }
  | { edit: "requirement"; node: string; requires: string };

function sourceTarget(text: Schema<"ConstraintSource">, view: Ready): EditTarget | undefined {
  const source = parseSource(text);
  if ("estimate" in source) {
    // A group's own estimate is no field: its contents set its length.
    const estimated = nodeOf(view, source.estimate.node)?.estimate !== undefined;
    return estimated ? { edit: "estimate", node: source.estimate.node } : undefined;
  }
  if ("due_by" in source) {
    return { edit: "due_by", node: source.due_by.node };
  }
  if ("not_before" in source) {
    return { edit: "not_before", node: source.not_before.node };
  }
  if ("dependency" in source) {
    const { node, requires, via } = source.dependency;
    if (via === "explicit") {
      return { edit: "requirement", node, requires };
    }
    if (typeof via === "object" && "inherited" in via) {
      return { edit: "requirement", node: via.inherited.ancestor, requires };
    }
  }
  return undefined;
}

function fixedTarget(fixed: FixedDate, view: Ready): EditTarget | undefined {
  const instant = parseInstant(fixed.instant);
  if (fixed.fixed_by === "answer" && "answer" in instant) {
    return { edit: "answer", decision: instant.answer };
  }
  if (fixed.fixed_by !== "pin" || !("node" in instant)) {
    return undefined;
  }
  // E3: a milestone pinned by a feeding decision is edited only through that decision.
  const decision = feedingDecision(view, instant.node);
  return decision === undefined ? { edit: "pin", node: instant.node } : { edit: "answer", decision: decision.key };
}

/** F7: the pins and rules to edit to change a bound produced by `chain`, each once. */
export function editTargets(chain: Chain, view: Ready): EditTarget[] {
  const found = [
    ...(chain.fixed ?? []).map((fixed) => fixedTarget(fixed, view)),
    ...chain.constraints.map((constraint) => sourceTarget(constraint.source, view)),
  ];
  const targets: EditTarget[] = [];
  for (const target of found) {
    if (target !== undefined && !targets.some((held) => JSON.stringify(held) === JSON.stringify(target))) {
      targets.push(target);
    }
  }
  return targets;
}

/** An edit target, in words. */
export function targetText(target: EditTarget, name: Namer): string {
  switch (target.edit) {
    case "pin":
      return `the pin on ${name(target.node)}`;
    case "answer":
      return `the answer to ${name(target.decision)}`;
    case "due_by":
    case "not_before":
    case "estimate":
      return `the ${fieldWords(target.edit)} of ${name(target.node)}`;
    case "requirement":
      return `${name(target.node)}'s requirement on ${name(target.requires)}`;
  }
}

/** Why finished work is stale (D4), as the clause after the node's name: "needs a note", "Kickoff was reopened". */
export function staleText(reason: Schema<"StaleReason">, name: Namer, item: (id: string) => string): string {
  if (reason === "missing_artifact") {
    return "needs a link";
  }
  if (reason === "missing_note") {
    return "needs a note";
  }
  if (reason === "not_broken_down") {
    return "was not broken down";
  }
  if ("unticked" in reason) {
    return `needs "${item(reason.unticked)}" ticked`;
  }
  return `${name(reason.reopened)} was reopened`;
}

/** Where a participation comes from (E2), in words. */
export function originText(origin: Schema<"ParticipationOrigin">, view: Ready): string {
  if (origin === "explicit") {
    return "set on this node";
  }
  if ("role" in origin) {
    return `the role ${roleTitle(view, origin.role)}`;
  }
  if ("ancestor" in origin) {
    return `from ${titleOf(view, origin.ancestor)}`;
  }
  return `the default owner, ${roleTitle(view, origin.default_owner)}`;
}
