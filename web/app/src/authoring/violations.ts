// A15: a rejection lists every violation by path, and a form shows each one at the field it
// is about. The engine names the field where one field is at fault (`at.field`) and the
// mutation that touched the node; an edge it names is the node's requirements. A violation on
// another node, or one no field owns, is listed with the form as a whole.

import { violationNode, type Violation } from "../data/writes.ts";
import type { FormField } from "./fields.ts";
import type { Mutation } from "./graph.ts";


/** Where in a form a violation shows: a field, the node's requirements, or the form as a whole. */
export type Place = FormField | "requires" | "form";

/** Whether `violation` is about `node`: the engine reports a rule several nodes break together once at each of them. */
export function isAbout(violation: Violation, node: string): boolean {
  return violationNode(violation) === node;
}

/**
 * Where a violation about the form's node shows: the field the engine names, else the field
 * whose mutation it rejected (`sent` is each sent mutation's field, by position), else the
 * node's requirements when the violation names an edge, else the form as a whole.
 */
export function placeOf(violation: Violation, sent: readonly (Place | undefined)[] = []): Place {
  if (violation.at.field != null) {
    return violation.at.field;
  }
  const mutation = violation.at.mutation;
  const byMutation = mutation == null ? undefined : sent[mutation];
  if (byMutation !== undefined) {
    return byMutation;
  }
  return (violation.related ?? []).some((related) => typeof related === "object" && "edge" in related) ? "requires" : "form";
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
 * for an edge; none for the rest. Worked out from the
 * patch itself, so a rejection shown again lands at the same fields.
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
