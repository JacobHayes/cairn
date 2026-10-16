// A15: a rejection lists every violation by path, and a form shows each one at the field it
// is about. The engine names the field where one field is at fault (`at.field`); otherwise the
// violation's code says which (a duplicate id is the id; a second final milestone, `final`;
// an edge to an ancestor, the node's requirements). A violation on another node, or one no
// field owns, is listed with the form as a whole. Which node a violation is about is its
// subject or, for a rule two nodes break together, one of its related nodes.
import type { Schema } from "@cairn/client";

import type { FormField } from "./fields.ts";
import type { Mutation } from "./graph.ts";

export type Violation = Schema<"Violation">;

/** Where in a form a violation shows: a field, the node's requirements, or the form as a whole. */
export type Place = FormField | "requires" | "form";

const BY_CODE: Partial<Record<Schema<"ViolationCode">, Place>> = {
  duplicate_sibling_id: "id",
  leaf_with_children: "parent",
  containment_cycle: "parent",
  dependency_cycle: "requires",
  edge_to_ancestor_or_descendant: "requires",
  requires_duplicates_condition: "requires",
  condition_answer_type_mismatch: "relevant_when",
  condition_on_own_subtree: "relevant_when",
  stage_bound_not_milestone: "opens_at",
  several_final_milestones: "final",
  several_filling_decisions: "fills_role",
  several_feeding_decisions: "feeds_milestone",
  feeds_milestone_not_milestone: "feeds_milestone",
  fills_role_cardinality: "fills_role",
  answer_type_mismatch: "answer_type",
};

/** The node a violation is about: its subject, if a node. */
export function subjectNode(violation: Violation): string | undefined {
  const subject = violation.at.subject;
  return subject != null && typeof subject === "object" && "node" in subject ? subject.node : undefined;
}

/** Whether `violation` is about `node`: its subject, or one of the nodes it relates for a rule broken together. */
export function isAbout(violation: Violation, node: string): boolean {
  if (subjectNode(violation) === node) {
    return true;
  }
  const together = ["duplicate_sibling_id", "several_final_milestones", "several_filling_decisions", "several_feeding_decisions"];
  return together.includes(violation.code) && (violation.related ?? []).some((related) => typeof related === "object" && "node" in related && related.node === node);
}

/**
 * Where a violation about the form's node shows: the field the engine names, else the field
 * whose mutation it rejected (`sent` is each sent mutation's field, by position), else its code's.
 */
export function placeOf(violation: Violation, sent: readonly (Place | undefined)[] = []): Place {
  if (violation.at.field != null) {
    return violation.at.field;
  }
  const mutation = violation.at.mutation;
  const byMutation = mutation == null ? undefined : sent[mutation];
  return byMutation ?? BY_CODE[violation.code] ?? "form";
}

/** A15: the violations about `node`, by where each shows; the rest under "form". */
export function byPlace(violations: readonly Violation[], node: string | undefined, sent: readonly (Place | undefined)[] = []): Map<Place, Violation[]> {
  const places = new Map<Place, Violation[]>();
  for (const violation of violations) {
    const place = node !== undefined && isAbout(violation, node) ? placeOf(violation, sent) : "form";
    places.set(place, [...(places.get(place) ?? []), violation]);
  }
  return places;
}

/**
 * Where each mutation of a patch shows when the engine rejects it (A15), by position: the field
 * a `set_node_field` writes, the answer type a node replacement changes, a node's requirements
 * for an edge; none for the rest, which a violation's code places instead. Worked out from the
 * patch itself, so a rejection restored after a reload lands at the same fields.
 */
export function placesOf(mutations: readonly Mutation[]): (Place | undefined)[] {
  return mutations.map((mutation): Place | undefined => {
    switch (mutation.op) {
      case "set_node_field":
        return Object.keys(mutation.value)[0] as FormField | undefined;
      case "replace_node":
        return "answer_type";
      case "add_edge":
      case "remove_edge":
        return "requires";
      default:
        return undefined;
    }
  });
}
