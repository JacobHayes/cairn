// A15: a rejection's violations land at the fields they are about: the field the engine
// names, else the field of the mutation it rejected, else the field its code is about; a
// violation about another node stays with the form as a whole.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { byPlace, placeOf } from "./violations.ts";

type Violation = Schema<"Violation">;

const on = (node: string, code: Schema<"ViolationCode">, at: Partial<Violation["at"]> = {}, related: Schema<"Subject">[] = []): Violation => ({
  code,
  at: { subject: { node }, ...at },
  message: code,
  related,
});

describe("byPlace (A15)", () => {
  it("puts three violations of one node at three fields", () => {
    const places = byPlace(
      [on("n_stage", "duplicate_sibling_id"), on("n_stage", "edge_to_ancestor_or_descendant", { field: "opens_at" }), on("n_stage", "edge_to_ancestor_or_descendant", { field: "closes_at" })],
      "n_stage",
    );
    expect([...places.keys()].sort()).toEqual(["closes_at", "id", "opens_at"]);
  });

  it("finds the node of a rule two nodes break together among the related nodes", () => {
    const second = on("n_first", "several_final_milestones", {}, [{ node: "n_second" }]);
    expect([...byPlace([second], "n_second").keys()]).toEqual(["final"]);
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
