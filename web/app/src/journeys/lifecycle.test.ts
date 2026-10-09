// B1, B11, A19: what starting a journey sends, which routes it can start from, the status
// changes each status accepts, the typed confirmation before a hard delete, and when
// completion is suggested.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { STATUSES } from "./address.ts";
import { completionSuggested, createMutation, deleteConfirmed, newJourneyId, startableRoutes, statusActions, type RouteSummary } from "./lifecycle.ts";

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

  it("starts only from routes that are not retired or segments and have a published version (A19, A21)", () => {
    const route = (id: string, retired: boolean, latest: number | null, kind: "process" | "segment" = "process"): RouteSummary => ({ header: { id, name: id, retired, kind }, revision: 1, draft_open: false, latest_version: latest });
    const offered = startableRoutes([route("open", false, 2), route("retired", true, 3), route("unpublished", false, null), route("piece", false, 1, "segment")]);
    expect(offered.map((each) => each.header.id)).toEqual(["open"]);
  });
});

describe("a journey's status (B11, A19)", () => {
  /** The engine's legal moves (crates/engine, lifecycle `set_status`). */
  const LEGAL: Record<string, string[]> = { active: ["completed", "archived"], completed: ["active", "archived"], archived: ["completed"] };

  it.each(STATUSES)("offers from %s exactly the moves the engine accepts", (status) => {
    expect(statusActions(status).map((action) => action.to).sort()).toEqual([...(LEGAL[status] ?? [])].sort());
  });

  it("deletes only an archived journey, and only with its name typed back", () => {
    expect(deleteConfirmed("archived", "Launch", " Launch ")).toBe(true);
    expect(deleteConfirmed("archived", "Launch", "launch")).toBe(false);
    expect(deleteConfirmed("completed", "Launch", "Launch")).toBe(false);
  });
});

describe("completion suggested (B11)", () => {
  const summary = (remaining: number): Schema<"StatusSummary"> => ({ by_state: {}, by_display_state: {}, remaining });
  const derived = (autoReached: boolean): Schema<"Derived"> => ({
    today: "2026-10-06",
    frontier: [],
    acting_frontier: [],
    nodes: autoReached ? ({ n_end: { auto_reached: true } } as unknown as Schema<"Derived">["nodes"]) : {},
  });
  const graph = (state: string | undefined): Schema<"Graph"> =>
    ({
      nodes: [{ key: "n_end", id: "end", kind: "milestone", title: "End", final: true }],
      state: state === undefined ? {} : { nodes: { n_end: { state, provenance: "from_route" } } },
    }) as unknown as Schema<"Graph">;

  it.each([
    ["nothing in scope is left", graph(undefined), derived(false), summary(0), true],
    ["work is left and the final milestone is pending", graph("pending"), derived(false), summary(2), false],
    ["the final milestone is reached", graph("reached"), derived(false), summary(2), true],
    ["the final milestone reads as reached", graph("pending"), derived(true), summary(2), true],
  ])("when %s", (_, held, derive, counted, expected) => {
    expect(completionSuggested(held, derive, counted)).toBe(expected);
  });
});
