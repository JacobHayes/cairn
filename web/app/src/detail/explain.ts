// The words behind node detail's explanations (C8, F7): each derived value's explanation
// inputs from the local derive, said in terms of the journey's nodes, and, for a date bound,
// the pin or rule to edit to change it. Pure functions of the derived document, so every
// screen that explains a value (5.2's trace, 5.4's timeline) says it the same way.
import type { Schema } from "@cairn/client";

import { isLeaf } from "../authoring/graph.ts";
import { feedingDecision, nodeOf, titleOf, type Ready } from "./model.ts";
import { dateWords, originWord } from "../status/when.ts";
import { fieldWords, upperFirst } from "../status/words.ts";

import { roleTitle } from "./sections.tsx";

export type Bound = Schema<"Bound">;
export type Chain = Schema<"Chain">;
export type Constraint = Schema<"Constraint">;
export type ConstraintSource = Schema<"ConstraintSource">;
export type FixedDate = Schema<"FixedDate">;
export type Instant = Schema<"Instant">;
export type DependencyVia = Schema<"DependencyVia">;

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
export function instantText(instant: Instant, name: Namer): string {
  if (instant === "created_at") {
    return "the journey's creation";
  }
  if ("answer" in instant) {
    return `the answer to ${name(instant.answer.decision)}`;
  }
  return name(instant.node.node, instant.node.point);
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
  if ("condition" in via) {
    return `the condition on ${name(via.condition.condition_on)}`;
  }
  return `the opening of the stage ${name(via.stage_opening.group)}`;
}

/** Where a constraint comes from (F2), in words. */
export function sourceText(source: ConstraintSource, name: Namer): string {
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
  if ("containment" in source) {
    return `${name(source.containment.parent)} contains ${name(source.containment.child)}`;
  }
  return `the close of the stage ${name(source.stage_close.group)}`;
}

/** One constraint as the chain shows it: `after` at least `offset` days after `before`. */
export function constraintText(constraint: Constraint, name: Namer): string {
  const days = constraint.offset_days;
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
  | { edit: "requirement"; node: string; requires: string }
  | { edit: "stage_close"; group: string };

function sourceTarget(source: ConstraintSource, view: Ready): EditTarget | undefined {
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
  if ("stage_close" in source) {
    return { edit: "stage_close", group: source.stage_close.group };
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
  const instant = fixed.instant;
  if (fixed.fixed_by === "answer" && typeof instant === "object" && "answer" in instant) {
    return { edit: "answer", decision: instant.answer.decision };
  }
  if (fixed.fixed_by !== "pin" || typeof instant !== "object" || !("node" in instant)) {
    return undefined;
  }
  // E3: a milestone pinned by a feeding decision is edited only through that decision.
  const decision = feedingDecision(view, instant.node.node);
  return decision === undefined ? { edit: "pin", node: instant.node.node } : { edit: "answer", decision: decision.key };
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
    case "stage_close":
      return `the close of the stage ${name(target.group)}`;
  }
}

/** Why finished work is stale (D4), as the clause after the node's name: "needs a note", "Kickoff was reopened", "was finished before its date, Oct 12" (a date in `today`'s year leaves the year out). */
export function guardFailureText(failure: Schema<"GuardFailure">, name: Namer, today?: string): string {
  if (failure === "missing_artifact") {
    return "needs a link";
  }
  if (failure === "missing_note") {
    return "needs a note";
  }
  if (failure === "not_broken_down") {
    return "was not broken down";
  }
  if ("too_early" in failure) {
    return `was finished before its date, ${dateWords(failure.too_early, today)}`;
  }
  return `${name(failure.open_dependency)} was reopened`;
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
