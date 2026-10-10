// The toolbar's active filters: each setting on a screen's address becomes one chip, and
// the chip's address is the same screen with only that setting off.
import { describe, expect, it } from "vitest";

import { activeFilters } from "./filters.ts";

describe("the active filters under the toolbar", () => {
  it("is empty when no filter is on, so the row disappears", () => {
    expect(activeFilters("j_a", "list", "", undefined)).toEqual([]);
    expect(activeFilters("j_a", "graph", "?decisions=1&frontier=1", undefined)).toEqual([]);
    expect(activeFilters("j_a", "timeline", "?decisions=1", undefined)).toEqual([]);
  });

  it("makes a chip for each setting on the frontier list, each undoing only itself and keeping the lens", () => {
    const chips = activeFilters("j_a", "list", "?frontier=1&mine=1&kind=action,milestone&flag=overdue&q=plan&sort=due", "n_x");
    expect(chips.map((chip) => chip.label)).toEqual(["mine", "overdue", "action", "milestone", '"plan"']);
    expect(chips[0]?.without).toBe("/journeys/j_a/list/nodes/n_x?frontier=1&flag=overdue&kind=action%2Cmilestone&q=plan&sort=due");
    expect(chips[1]?.without).toBe("/journeys/j_a/list/nodes/n_x?frontier=1&mine=1&kind=action%2Cmilestone&q=plan&sort=due");
  });

  it("keeps a chip for a flag the plan list's panel no longer offers, beside the ones chosen in it (2.3)", () => {
    const chips = activeFilters("j_a", "list", "?flag=next_up,stale", undefined);
    expect(chips.map((chip) => chip.label)).toEqual(["next up", "stale"]);
    expect(chips[0]?.without).toBe("/journeys/j_a/list?flag=stale");
  });

  it("makes a chip for the kinds the canvas keeps in focus and for conditional nodes it turns off", () => {
    const chips = activeFilters("j_a", "graph", "?kind=group,decision&show=notrelevant", undefined);
    expect(chips.map((chip) => chip.label)).toEqual(["only group, decision", "no conditional"]);
    expect(chips[1]?.without).toBe("/journeys/j_a/graph?kind=group%2Cdecision&show=notrelevant%2Cconditional");
  });
});
