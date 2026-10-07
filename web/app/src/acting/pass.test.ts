// C11: a triage pass reorders the current frontier for this pass only: passed cards go to the
// back in the order passed, new ones surface where they rank, and what surfaced is named.
import { describe, expect, it } from "vitest";

import { begin, passedAll, passOn, passOrder, surfaced } from "./pass.ts";

const ranked = ["n_a", "n_b", "n_c", "n_d"];

describe("a triage pass", () => {
  it("starts in rank order", () => {
    expect(passOrder(ranked, begin(ranked))).toEqual(ranked);
  });

  it("sends a passed card to the back, in the order passed", () => {
    const pass = passOn(passOn(begin(ranked), "n_b"), "n_a");
    expect(passOrder(ranked, pass)).toEqual(["n_c", "n_d", "n_b", "n_a"]);
  });

  it("moves a card passed again behind the others passed", () => {
    const pass = passOn(passOn(passOn(begin(ranked), "n_a"), "n_b"), "n_a");
    expect(pass.passed).toEqual(["n_b", "n_a"]);
  });

  it("reads the current frontier: a finished card leaves, a new one takes its rank", () => {
    const pass = passOn(begin(ranked), "n_a");
    const now = ["n_new", "n_a", "n_c", "n_d"];
    expect(passOrder(now, pass)).toEqual(["n_new", "n_c", "n_d", "n_a"]);
    expect(surfaced(now, pass)).toEqual(["n_new"]);
  });

  it("is finished once every card showing has been passed", () => {
    const pass = ranked.reduce(passOn, begin(ranked));
    expect(passedAll(ranked, pass)).toBe(true);
    expect(passedAll(["n_new", ...ranked], pass)).toBe(false);
    expect(passedAll([], pass)).toBe(false);
  });
});
