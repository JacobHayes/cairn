// A15: a rejection's violations land at the fields they are about: the field the engine
// names, else the field of the mutation it rejected, else the field its code is about; a
// violation about another node stays with the form as a whole.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { byPlace, placeOf, placesOf } from "./violations.ts";

type Violation = Schema<"Violation">;

const on = (node: string, code: Schema<"ViolationCode">, at: Partial<Violation["at"]> = {}, related: Schema<"Subject">[] = []): Violation => ({
  code,
  at: { subject: { node }, ...at },
  message: code,
  related,
});

describe("byPlace (A15)", () => {
  it("puts violations of one node at their fields", () => {
    const places = byPlace(
      [on("n_stage", "duplicate_sibling_id", { field: "id" }), on("n_stage", "edge_to_ancestor_or_descendant", { field: "due_by" })],
      "n_stage",
    );
    expect([...places.keys()].sort()).toEqual(["due_by", "id"]);
  });

  it("puts a violation that names an edge at the node's requirements", () => {
    const edge = on("n_mine", "edge_to_ancestor_or_descendant", {}, [{ edge: { node: "n_mine", requires: "n_up" } }]);
    expect([...byPlace([edge], "n_mine").keys()]).toEqual(["requires"]);
  });

  it("keeps a violation about another node, or none, with the form as a whole", () => {
    const places = byPlace([on("n_other", "dangling_reference"), { code: "limit_exceeded", at: {}, message: "" }], "n_mine");
    expect([...places.keys()]).toEqual(["form"]);
    expect(places.get("form")).toHaveLength(2);
  });

  it("places a mutation's violation at the field that sent it", () => {
    expect(placeOf(on("n_mine", "field_not_on_kind", { mutation: 1 }), ["title", "estimate"])).toBe("estimate");
  });
});

describe("placesOf (A15)", () => {
  it("names the field each mutation writes, from the mutations alone, and none for the rest", () => {
    const places = placesOf([
      { op: "set_node_field", node: "n_a", value: { estimate: 2 } },
      { op: "replace_node", node: { key: "n_a" } } as never,
      { op: "add_edge", edge: { node: "n_a", requires: "n_b" } } as never,
      { op: "remove_edge", edge: { node: "n_a", requires: "n_b" } } as never,
      { op: "discard_draft" },
    ]);
    expect(places).toEqual(["estimate", "answer_type", "requires", "requires", undefined]);
  });

  it("lets a violation of a restored patch land at the field its mutation wrote", () => {
    const places = placesOf([{ op: "set_node_field", node: "n_mine", value: { weight: 3 } }]);
    expect(placeOf(on("n_mine", "limit_exceeded", { mutation: 0 }), places)).toBe("weight");
  });
});
