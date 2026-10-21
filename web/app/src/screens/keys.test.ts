// The keyboard map's decisions: which row `j` and `k` move to, and which step `1` to `4` picks.
import { describe, expect, it } from "vitest";

import { unavailableScreens } from "../journeys/address.ts";
import { JOURNEY_OFFERS, ROUTE_OFFERS } from "../journeys/view.ts";
import { keyBindingsFor, rowAfter, stepForKey } from "./keys.ts";

describe("j and k", () => {
  it("move to the next and the previous row, stop at the ends, and start at the first or last row when nothing is selected", () => {
    const keys = ["a", "b", "c"];
    expect(rowAfter(keys, "a", 1)).toBe("b");
    expect(rowAfter(keys, "c", -1)).toBe("b");
    expect(rowAfter(keys, "c", 1)).toBeUndefined();
    expect(rowAfter(keys, "a", -1)).toBeUndefined();
    expect(rowAfter(keys, undefined, 1)).toBe("a");
    expect(rowAfter(keys, undefined, -1)).toBe("c");
    expect(rowAfter(keys, "gone", 1)).toBe("a");
    expect(rowAfter([], undefined, 1)).toBeUndefined();
  });
});

describe("1 to 4", () => {
  it("pick Stages to Actions, only the steps the journey offers", () => {
    const every = ["stages", "decisions", "deliverables", "actions"] as const;
    expect(["1", "2", "3", "4"].map((key) => stepForKey(key, every))).toEqual(every);
    expect(stepForKey("1", ["decisions", "deliverables", "actions"])).toBeUndefined();
    expect(stepForKey("5", every)).toBeUndefined();
    expect(stepForKey("d", every)).toBeUndefined();
  });
});

describe("the key sheet", () => {
  const keys = (...args: Parameters<typeof keyBindingsFor>) => keyBindingsFor(...args).map((binding) => binding.keys);

  it("lists only the keys that act on the page: a route has no Frontier, Only mine or Walkthrough", () => {
    expect(keys(JOURNEY_OFFERS)).toEqual(expect.arrayContaining(["Shift F", "m", "w", "p"]));
    for (const dead of ["Shift F", "m", "w", "p"]) {
      expect(keys(ROUTE_OFFERS)).not.toContain(dead);
    }
  });

  it("leaves out the Walkthrough's keys under a proposal, where the Walkthrough is unavailable", () => {
    const under = keys(JOURNEY_OFFERS, { unavailable: unavailableScreens({ journey: "j_a", proposal: "p_1" }) });
    expect(keys(JOURNEY_OFFERS, { unavailable: unavailableScreens({ journey: "j_a" }) })).toContain("w");
    expect(under).not.toContain("w");
    expect(under).not.toContain("p");
    expect(under).toContain("Shift F");
  });

  it("leaves out what the screen does not act on: rows off the list, the view keys on the Summary, Frontier on the Walkthrough, Enter where nodes take no action", () => {
    expect(keys(ROUTE_OFFERS, { screen: "list" })).toContain("j / k");
    expect(keys(ROUTE_OFFERS, { screen: "graph" })).not.toContain("j / k");
    const summary = keys(JOURNEY_OFFERS, { screen: "summary" });
    for (const dead of ["1 - 4", "/", "m", "Shift F"]) {
      expect(summary).not.toContain(dead);
    }
    expect(keys(JOURNEY_OFFERS, { screen: "walk" })).not.toContain("Shift F");
    expect(keys(JOURNEY_OFFERS, { screen: "graph" })).toEqual(expect.arrayContaining(["1 - 4", "/", "m", "Shift F", "Enter"]));
    expect(keys(JOURNEY_OFFERS, { screen: "graph", acts: false })).not.toContain("Enter");
  });
});
