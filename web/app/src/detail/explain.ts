// The words behind node detail's explanations (C8, F7): each derived value's explanation
// inputs from the local derive, said in terms of the journey's nodes, and, for a date bound,
// the pin or rule to edit to change it. Pure functions of the derived document, so every
// screen that explains a value (5.2's trace, 5.4's timeline) says it the same way.
import type { Schema } from "@cairn/client";

import { feedingDecision, nodeOf, titleOf, type Mutation, type Ready } from "./model.ts";

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
    const single = kind === "decision" || kind === "milestone";
    return point === undefined || single ? titleOf(view, key) : `${titleOf(view, key)} ${point}`;
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
    return `the due_by rule of ${name(source.due_by.node)}`;
  }
  if ("not_before" in source) {
    return `the not_before rule of ${name(source.not_before.node)}`;
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
  const offset = days === 0 ? "" : days > 0 ? ` + ${String(days)} days` : ` - ${String(-days)} days`;
  return `${instantText(constraint.after, name)} no earlier than ${instantText(constraint.before, name)}${offset}`;
}

/** What fixes a date on a chain, in words. */
export function fixedText(fixed: FixedDate, name: Namer): string {
  const by = { pin: "pinned", actual: "actual", answer: "answered", today: "today" }[fixed.fixed_by];
  return `${instantText(fixed.instant, name)}: ${fixed.date} (${by})`;
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
      return `the ${target.edit} of ${name(target.node)}`;
    case "requirement":
      return `${name(target.node)}'s requirement on ${name(target.requires)}`;
    case "stage_close":
      return `the close of the stage ${name(target.group)}`;
  }
}

/** A guard failure (D4) as `stale` lists it. */
export function guardFailureText(failure: Schema<"GuardFailure">, name: Namer): string {
  if (failure === "missing_artifact") {
    return "missing artifact";
  }
  if (failure === "not_broken_down") {
    return "not broken down";
  }
  return `open dependency: ${name(failure.open_dependency)}`;
}

/** Where a participation comes from (E2), in words. */
export function originText(origin: Schema<"ParticipationOrigin">, view: Ready): string {
  if (origin === "explicit") {
    return "set on this node";
  }
  const roleTitle = (role: string) => (view.journey.graph.roles ?? []).find((each) => each.key === role)?.title ?? role;
  if ("role" in origin) {
    return `the role ${roleTitle(origin.role)}`;
  }
  if ("ancestor" in origin) {
    return `from ${titleOf(view, origin.ancestor)}`;
  }
  return `the default owner, ${roleTitle(origin.default_owner)}`;
}

/** F5: a resolution move (one ordinary mutation) in words. */
export function moveText(move: Mutation, name: Namer): string {
  switch (move.op) {
    case "shift_pin": {
      const days = move.offset_days;
      return `Move the pin on ${name(move.node)} ${String(Math.abs(days))} days ${days < 0 ? "earlier" : "later"}`;
    }
    case "set_pin":
      return `Pin ${name(move.node)} to ${move.date}`;
    case "clear_pin":
      return `Unpin ${name(move.node)}`;
    case "answer":
      return `Answer ${name(move.decision)} with ${answerWords(move.value)}`;
    case "remove_edge":
      return `Drop ${name(move.edge.node)}'s requirement on ${name(move.edge.requires)}`;
    case "set_node_field":
      return fieldMoveText(move.node, move.value, name);
    default:
      return move.op.replaceAll("_", " ");
  }
}

/** An answer's value, in words (choices by id; entities by key). */
export function answerWords(value: Schema<"AnswerValue">): string {
  const [kind, answer] = (Object.entries(value) as [string, unknown][])[0] ?? ["", undefined];
  if (Array.isArray(answer)) {
    return answer.length === 0 ? "none" : answer.join(", ");
  }
  return kind === "boolean" ? (answer === true ? "yes" : "no") : typeof answer === "string" ? answer : JSON.stringify(answer);
}

/** F5: a rule, estimate, or stage close loosened, in words. */
function fieldMoveText(node: string, value: Schema<"NodeFieldValueResolved">, name: Namer): string {
  if ("closes" in value) {
    return `Let the stage ${name(node)} run past its close`;
  }
  if ("estimate" in value) {
    return `Shorten the estimate of ${name(node)} to ${String(value.estimate)} days`;
  }
  if ("due_by" in value || "not_before" in value) {
    const [field, rule] = "due_by" in value ? ["due_by", value.due_by] : ["not_before", value.not_before];
    return `Change the ${field} rule of ${name(node)} to an offset of ${String(rule?.offset ?? 0)} days`;
  }
  return `Change ${Object.keys(value).join(", ")} of ${name(node)}`;
}
