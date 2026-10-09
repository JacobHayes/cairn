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

  it("makes a chip for each setting on the next list, each undoing only itself", () => {
    const chips = activeFilters("j_a", "next", "list", "?mine=1&kind=action,milestone&flag=overdue&q=plan&sort=due&me=1", "n_x");
    // Ranking for me is a sort, not a filter: no chip.
    expect(chips.map((chip) => chip.label)).toEqual(["mine", "action", "milestone", "overdue", '"plan"']);
    expect(chips[0]?.without).toBe("/journeys/j_a/next/list/nodes/n_x?sort=due&kind=action%2Cmilestone&flag=overdue&me=1&q=plan");
    expect(chips[3]?.without).toBe("/journeys/j_a/next/list/nodes/n_x?sort=due&mine=1&kind=action%2Cmilestone&me=1&q=plan");
  });

  it("makes a chip for the kinds the canvas keeps in focus and for conditional nodes it turns off", () => {
    const chips = activeFilters("j_a", "plan", "graph", "?kind=group,decision&show=notrelevant", undefined);
    expect(chips.map((chip) => chip.label)).toEqual(["only group, decision", "no conditional"]);
    expect(chips[1]?.without).toBe("/journeys/j_a/plan/graph?kind=group%2Cdecision&show=notrelevant%2Cconditional");
  });
});
