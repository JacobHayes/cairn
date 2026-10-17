// C1, C2, C9, C10, C11: the view state every journey view shares: it survives the address, a
// switch of view carries all of it and nothing of the view left, and picking a depth drops the
// viewer's expands and collapses. The chips take one setting off each; Reset keeps Frontier and Fade.
import { describe, expect, it } from "vitest";

import { DEFAULT_FILTER, DEFAULT_VIEW_STATE, filterChanged, filterChips, isStock, stockView, viewStateFrom, viewStateParams, withViewState, type ViewState } from "./view.ts";

const narrowed: ViewState = {
  depth: "decisions",
  fade: true,
  frontier: true,
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
    ["Stages and a search", { ...DEFAULT_VIEW_STATE, depth: "stages", text: "two words" }],
  ])("round-trips %s", (_, state) => {
    expect(viewStateFrom(viewStateParams(state))).toEqual(state);
  });

  it("leaves the stock view, and the Actions depth, bare", () => {
    expect(viewStateParams(DEFAULT_VIEW_STATE).toString()).toBe("");
    expect(read("depth=actions")).toEqual(DEFAULT_VIEW_STATE);
  });

  it("writes Fade only when it is on", () => {
    expect(viewStateParams({ ...DEFAULT_VIEW_STATE, fade: true }).get("fade")).toBe("1");
    expect(viewStateParams(DEFAULT_VIEW_STATE).has("fade")).toBe(false);
  });

  it("reads no depth or Decisions from the parameters of the earlier controls: they are not aliases", () => {
    expect(read("detail=decisions&decisions=1")).toEqual(DEFAULT_VIEW_STATE);
    expect(read("depth=work")).toEqual(DEFAULT_VIEW_STATE);
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

describe("changing the view state", () => {
  it("keeps the view's own parameters and the viewer's folds while the depth holds", () => {
    const next = new URLSearchParams(withViewState("?depth=deliverables&open=n_a&sort=due", { ...DEFAULT_VIEW_STATE, depth: "deliverables", frontier: true }));
    expect([next.get("frontier"), next.get("open"), next.get("sort")]).toEqual(["1", "n_a", "due"]);
  });

  it("drops the viewer's expands and collapses when the depth changes or a depth is picked", () => {
    const changed = new URLSearchParams(withViewState("?depth=deliverables&open=n_a&shut=n_b", { ...DEFAULT_VIEW_STATE, depth: "stages" }));
    expect([changed.get("open"), changed.get("shut")]).toEqual([null, null]);
    const picked = new URLSearchParams(withViewState("?depth=deliverables&open=n_a", { ...DEFAULT_VIEW_STATE, depth: "deliverables" }, true));
    expect(picked.get("open")).toBeNull();
  });

  it("keeps the viewer's folds when the depth changes under Fade, which only dims, until Reset says picked", () => {
    const stepped = new URLSearchParams(withViewState("?depth=actions&fade=1&open=n_a&shut=n_b", { ...DEFAULT_VIEW_STATE, depth: "decisions", fade: true }));
    expect([stepped.get("open"), stepped.get("shut")]).toEqual(["n_a", "n_b"]);
    const reset = new URLSearchParams(withViewState("?depth=decisions&fade=1&open=n_a", { ...DEFAULT_VIEW_STATE, fade: true }, true));
    expect(reset.get("open")).toBeNull();
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

  it("keeps the viewer's folds when Fade is toggled, since the depth holds", () => {
    const faded = new URLSearchParams(withViewState("?depth=decisions&open=n_a&shut=n_b", { ...DEFAULT_VIEW_STATE, depth: "decisions", fade: true }));
    expect([faded.get("fade"), faded.get("open"), faded.get("shut")]).toEqual(["1", "n_a", "n_b"]);
  });

  it("resets to the stock view but keeps Frontier and Fade, which hide nothing", () => {
    const reset = stockView(narrowed);
    expect(isStock(reset)).toBe(true);
    expect([reset.frontier, reset.fade]).toEqual([true, true]);
    expect(isStock(narrowed)).toBe(false);
  });

  it("presses the Filter for a class switch alone", () => {
    expect(filterChanged(DEFAULT_FILTER)).toBe(false);
    expect(filterChanged({ ...DEFAULT_FILTER, conditional: false })).toBe(true);
    expect(filterChanged({ ...DEFAULT_FILTER, notRelevant: true })).toBe(true);
  });
});
