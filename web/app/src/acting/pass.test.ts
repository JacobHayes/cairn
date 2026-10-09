// C11: a triage pass reorders the current frontier for this pass only: passed cards go to the
// back in the order passed, new ones surface where they rank, and what surfaced is named; what
// an action unlocked comes next, the latest action's first.
import { describe, expect, it } from "vitest";

import { acted, begin, doneThisPass, passedAll, passOn, passOrder, roundAgain, surfaced, unlockedBy } from "./pass.ts";

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

  it("goes round again in rank order, still naming what surfaced since it began", () => {
    const pass = roundAgain(ranked.reduce(passOn, begin(ranked)));
    const now = ["n_new", ...ranked];
    expect(passOrder(now, pass)).toEqual(now);
    expect(surfaced(now, pass)).toEqual(["n_new"]);
  });

  it("puts what an action unlocked next, in rank order, and a later action's ahead of an earlier one's", () => {
    // n_a is answered and unlocks n_d and n_c; then n_d is acted on and unlocks n_e.
    const first = acted(begin(ranked), "n_a", ["n_d", "n_c"]);
    expect(passOrder(["n_b", "n_c", "n_d"], first)).toEqual(["n_c", "n_d", "n_b"]);
    const second = acted(first, "n_d", ["n_e"]);
    const now = ["n_b", "n_c", "n_e"];
    expect(passOrder(now, second)).toEqual(["n_e", "n_c", "n_b"]);
    expect(unlockedBy(second, "n_e")).toBe("n_d");
    expect(unlockedBy(second, "n_d")).toBeUndefined();
    // n_e was not on the frontier when the pass began; finished from its card, it is still done in this pass.
    expect(doneThisPass(acted(second, "n_e", []), (key) => key === "n_a" || key === "n_e")).toEqual(["n_a", "n_e"]);
  });

  it("drops an unlocked card's label once it is acted on or passed, for good", () => {
    const pass = passOn(acted(begin(ranked), "n_a", ["n_c", "n_d"]), "n_c");
    expect(unlockedBy(pass, "n_c")).toBeUndefined();
    expect(passOrder(["n_b", "n_c", "n_d"], pass)).toEqual(["n_d", "n_b", "n_c"]);
    expect(passOrder(["n_b", "n_c", "n_d"], roundAgain(pass))).toEqual(["n_d", "n_b", "n_c"]);
  });

  it("skips an unlocked card that has left the frontier before it is reached", () => {
    const pass = acted(begin(ranked), "n_a", ["n_x", "n_c"]);
    expect(passOrder(["n_b", "n_c"], pass)).toEqual(["n_c", "n_b"]);
  });
});
