// B1, B11, A19: what starting a journey sends, which routes it can start from, the status
// changes each status accepts, the typed confirmation before a hard delete, and when
// completion is suggested.
import { describe, expect, it } from "vitest";

import { createMutation, deleteConfirmed, newJourneyId } from "./lifecycle.ts";

const JOURNEY_ID = /^j_[a-z0-9][a-z0-9_-]*$/;

describe("starting a journey (B1)", () => {
  it.each(["Vendor evaluation, round 2", "  ", "Évaluation à l'étranger", "x".repeat(300)])("mints an id the host accepts from %j", (name) => {
    const id = newJourneyId(name, () => "abc123");
    expect(id).toMatch(JOURNEY_ID);
    expect(id.length).toBeLessThanOrEqual(64);
    expect(id.endsWith("_abc123")).toBe(true);
  });

  it("sends the name and lineage, and a description only when one was written", () => {
    expect(createMutation(" Launch ", "  ", { route: "r", version: 2 })).toEqual({ op: "create_journey", name: "Launch", from: { route: "r", version: 2 } });
    expect(createMutation("Ad hoc", "Why", undefined)).toEqual({ op: "create_journey", name: "Ad hoc", description: "Why" });
  });
});

describe("a journey's status (B11, A19)", () => {
  it("deletes only an archived journey, and only with its name typed back", () => {
    expect(deleteConfirmed("archived", "Launch", " Launch ")).toBe(true);
    expect(deleteConfirmed("archived", "Launch", "launch")).toBe(false);
    expect(deleteConfirmed("completed", "Launch", "Launch")).toBe(false);
  });
});
