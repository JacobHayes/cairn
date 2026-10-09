// The toolbar's active filters: each setting on a projection's address becomes one chip, and
// the chip's address is the same screen with only that setting off.
import { describe, expect, it } from "vitest";

import { activeFilters } from "./filters.ts";

describe("the active filters under the toolbar", () => {
  it("is empty when no filter is on, so the row disappears", () => {
    expect(activeFilters("j_a", "next", "list", "", undefined)).toEqual([]);
    expect(activeFilters("j_a", "plan", "graph", "?decisions=1", undefined)).toEqual([]);
    expect(activeFilters("j_a", "plan", "timeline", "?decisions=1", undefined)).toEqual([]);
  });

  it("makes a chip for each kind and the search on the timeline, none for the kinds under DECISIONS", () => {
    const chips = activeFilters("j_a", "plan", "timeline", "?kind=action,milestone&q=plan", "n_x");
    expect(chips.map((chip) => chip.label)).toEqual(["action", "milestone", '"plan"']);
    expect(chips[0]?.without).toBe("/journeys/j_a/plan/timeline/nodes/n_x?kind=milestone&q=plan");
    expect(chips[1]?.without).toBe("/journeys/j_a/plan/timeline/nodes/n_x?kind=action&q=plan");
    expect(activeFilters("j_a", "plan", "timeline", "?decisions=1&kind=action", undefined)).toEqual([]);
  });

  it("makes a chip for each setting on the next list, each undoing only itself", () => {
    const chips = activeFilters("j_a", "next", "list", "?mine=1&kind=action,milestone&flag=overdue&q=plan&sort=due&me=1", "n_x");
    // Ranking for me is a sort, not a filter: no chip.
    expect(chips.map((chip) => chip.label)).toEqual(["mine", "action", "milestone", "overdue", '"plan"']);
    expect(chips[0]?.without).toBe("/journeys/j_a/next/list/nodes/n_x?sort=due&kind=action%2Cmilestone&flag=overdue&me=1&q=plan");
    expect(chips[3]?.without).toBe("/journeys/j_a/next/list/nodes/n_x?sort=due&mine=1&kind=action%2Cmilestone&me=1&q=plan");
  });

  it("makes a chip for each of the cards' filters, and none for the kinds in the walkthrough", () => {
    expect(activeFilters("j_a", "next", "cards", "?mine=1&kind=action", undefined).map((chip) => chip.label)).toEqual(["mine", "action"]);
    expect(activeFilters("j_a", "next", "cards", "?decisions=1&mine=1&kind=action", undefined).map((chip) => chip.label)).toEqual(["mine"]);
  });

  it("makes a chip for each of the plan list's flags, including mine, and keeps the DECISIONS chip on", () => {
    const chips = activeFilters("j_a", "plan", "list", "?decisions=1&mine=1&flag=overdue,stale&owner=e_lead", undefined);
    expect(chips.map((chip) => chip.label)).toEqual(["mine", "overdue", "stale", "owner"]);
    expect(chips[1]?.without).toBe("/journeys/j_a/plan/list?decisions=1&mine=1&flag=stale&owner=e_lead");
  });

  it("makes a chip for each kind and relevance the canvas leaves out", () => {
    const chips = activeFilters("j_a", "plan", "graph", "?kind=group,decision&show=notrelevant", undefined);
    expect(chips.map((chip) => chip.label)).toEqual(["no deliverables", "no actions", "no milestones", "no conditional"]);
    expect(chips[3]?.without).toBe("/journeys/j_a/plan/graph?kind=group%2Cdecision");
  });
});
