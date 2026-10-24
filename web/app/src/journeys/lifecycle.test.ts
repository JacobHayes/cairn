// B1, B11, A19: what starting a journey sends, which routes it can start from, the status
// changes each status accepts, what a hard delete of several sends and when it is offered, and
// when completion is suggested.
import { KEY_PREFIX } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { isKey } from "../authoring/keys.ts";

import { createMutation, deleteBlocked, deleteMutations, newJourneyId, statusActions, type Doomed } from "./lifecycle.ts";

describe("starting a journey (B1)", () => {
  it.each(["Vendor evaluation, round 2", "  ", "Évaluation à l'étranger", "x".repeat(300)])("mints an id the host accepts from %j", (name) => {
    const id = newJourneyId(name, () => "abc123");
    expect(isKey(KEY_PREFIX.JourneyId, id)).toBe(true);
    expect(id.length).toBeLessThanOrEqual(64);
    expect(id.endsWith("_abc123")).toBe(true);
  });

  it("sends the name and lineage, and a description only when one was written", () => {
    expect(createMutation(" Launch ", "  ", [], { route: "r", version: 2 })).toEqual({ op: "create_journey", name: "Launch", from: { route: "r", version: 2 } });
    expect(createMutation("Owned", "", ["who_a"], undefined)).toEqual({ op: "create_journey", name: "Owned", owners: ["who_a"] });
    expect(createMutation("Ad hoc", "Why", [], undefined)).toEqual({ op: "create_journey", name: "Ad hoc", description: "Why" });
  });
});

describe("deleting journeys (B11, A19)", () => {
  const journey = (id: string, status: Doomed["status"], revision: number): Doomed => ({ id, name: id.toUpperCase(), status, revision });

  it("names every journey at the revision it was read at, in one patch", () => {
    const journeys = [journey("j_a", "archived", 4), journey("j_b", "archived", 9)];
    expect(deleteMutations(journeys)).toEqual([
      { op: "delete_journey", journey: "j_a", revision: 4 },
      { op: "delete_journey", journey: "j_b", revision: 9 },
    ]);
    expect(deleteBlocked(journeys)).toBeUndefined();
  });

  it("is offered only when every journey is archived, naming the ones that are not", () => {
    const blocked = deleteBlocked([journey("j_a", "archived", 1), journey("j_b", "completed", 1), journey("j_c", "active", 1)]);
    expect(blocked).toContain("J_B");
    expect(blocked).toContain("J_C");
    expect(blocked).not.toContain("J_A");
  });
});

describe("status moves (B11)", () => {
  it("offers an archived journey both ways out, each under its own name", () => {
    const actions = statusActions("archived");
    expect(actions.map((action) => action.to).sort()).toEqual(["active", "completed"]);
    expect(new Set(actions.map((action) => action.label)).size).toBe(2);
  });
});
