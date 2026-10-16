// C1, C2, C9, C10, C11: the view state every journey view shares: it survives the address, a
// switch of view carries all of it and nothing of the view left, and picking a step drops the
// viewer's expands and collapses. The chips take one setting off each; Reset keeps Frontier.
import { describe, expect, it } from "vitest";

import { DEFAULT_FILTER, DEFAULT_VIEW_STATE, filterChanged, filterChips, isStock, stockView, switchedSearch, viewStateFrom, viewStateParams, VIEW_PARAMS, withViewState, type ViewState } from "./view.ts";

const narrowed: ViewState = {
  step: "decisions",
  frontier: true,
  decisions: true,
  filter: { mine: true, owner: "e_lead", flags: ["overdue", "stale"], notRelevant: true, conditional: false },
  text: "plan",
};

const read = (search: string) => viewStateFrom(new URLSearchParams(search));

describe("the view state in the address", () => {
  it.each<[string, ViewState]>([
    ["stock", DEFAULT_VIEW_STATE],
    ["every setting moved", narrowed],
    ["only the classes", { ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, notRelevant: true } }],
    ["neither class", { ...DEFAULT_VIEW_STATE, filter: { ...DEFAULT_FILTER, conditional: false } }],
    ["Stages and a search", { ...DEFAULT_VIEW_STATE, step: "stages", text: "two words" }],
  ])("round-trips %s", (_, state) => {
    expect(viewStateFrom(viewStateParams(state))).toEqual(state);
  });

  it("leaves the stock view, and the All step, bare", () => {
    expect(viewStateParams(DEFAULT_VIEW_STATE).toString()).toBe("");
    expect(read("detail=all")).toEqual(DEFAULT_VIEW_STATE);
  });

  it("reads show= absent as the conditional class shown and not relevant hidden, and present as the classes it lists", () => {
    expect([read("").filter.conditional, read("").filter.notRelevant]).toEqual([true, false]);
    expect([read("show=notrelevant").filter.conditional, read("show=notrelevant").filter.notRelevant]).toEqual([false, true]);
    expect([read("show=").filter.conditional, read("show=").filter.notRelevant]).toEqual([false, false]);
  });

  it("drops a search longer than the engine reads", () => {
    expect(read(`q=${"a".repeat(256)}`).text).toHaveLength(256);
    expect(read(`q=${"a".repeat(257)}`).text).toBe("");
  });

  it("drops the flags the Filter does not offer", () => {
    expect(read("flag=mine,blocked,overdue").filter.flags).toEqual(["overdue"]);
  });
});

describe("switching views", () => {
  const search = `?${viewStateParams(narrowed).toString()}&sort=due&cols=rank&range=month&open=n_a&lens=gravity`;

  it("carries every view setting to every other view and drops the view's own", () => {
    for (const to of ["graph", "timeline", "summary", "walk"] as const) {
      const carried = new URLSearchParams(switchedSearch(search, "list", to));
      expect(read(carried.toString())).toEqual(narrowed);
      expect([...carried.keys()].filter((name) => !(VIEW_PARAMS as readonly string[]).includes(name))).toEqual([]);
    }
  });

  it("keeps the query of the view already open", () => {
    expect(switchedSearch(search, "list", "list")).toBe(search);
  });
});

describe("changing the view state", () => {
  it("keeps the view's own parameters and the viewer's folds while the step holds", () => {
    const next = new URLSearchParams(withViewState("?detail=work&open=n_a&sort=due", { ...DEFAULT_VIEW_STATE, step: "work", frontier: true }));
    expect([next.get("frontier"), next.get("open"), next.get("sort")]).toEqual(["1", "n_a", "due"]);
  });

  it("drops the viewer's expands and collapses when the step changes or a step is picked", () => {
    const changed = new URLSearchParams(withViewState("?detail=work&open=n_a&shut=n_b", { ...DEFAULT_VIEW_STATE, step: "stages" }));
    expect([changed.get("open"), changed.get("shut")]).toEqual([null, null]);
    const picked = new URLSearchParams(withViewState("?detail=work&open=n_a", { ...DEFAULT_VIEW_STATE, step: "work" }, true));
    expect(picked.get("open")).toBeNull();
  });

  it("makes each chip take exactly its own setting off", () => {
    const chips = filterChips(narrowed, (key) => key);
    expect(chips.length).toBeGreaterThan(0);
    for (const chip of chips) {
      const changed = (Object.keys(narrowed.filter) as (keyof ViewState["filter"])[]).filter((name) => JSON.stringify(chip.without.filter[name]) !== JSON.stringify(narrowed.filter[name]));
      const text = chip.without.text !== narrowed.text;
      expect(changed.length + (text ? 1 : 0), chip.id).toBe(1);
      expect({ ...chip.without, filter: narrowed.filter, text: narrowed.text }).toEqual(narrowed);
    }
  });

  it("resets to the stock view but keeps Frontier, which hides nothing", () => {
    const reset = stockView(narrowed);
    expect(isStock(reset)).toBe(true);
    expect(reset.frontier).toBe(true);
    expect(isStock(narrowed)).toBe(false);
  });

  it("presses the Filter for a class switch alone", () => {
    expect(filterChanged(DEFAULT_FILTER)).toBe(false);
    expect(filterChanged({ ...DEFAULT_FILTER, conditional: false })).toBe(true);
    expect(filterChanged({ ...DEFAULT_FILTER, notRelevant: true })).toBe(true);
  });
});
