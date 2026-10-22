import { describe, expect, it } from "vitest";

import { mineCountOf } from "./mine.ts";

describe("what the index's mine counts in a journey (E4)", () => {
  const viewer = ["who_a"];
  it("counts the viewer's work", () => {
    expect(mineCountOf({}, 3, viewer)).toBe(3);
  });
  it("keeps a journey the viewer owns with no work in it", () => {
    expect(mineCountOf({ owners: ["who_a"] }, 0, viewer)).toBe(1);
    expect(mineCountOf({ owners: ["who_a"] }, 4, viewer)).toBe(4);
  });
  it("leaves out a journey neither owned nor worked on by the viewer", () => {
    expect(mineCountOf({ owners: ["who_b"] }, 0, viewer)).toBe(0);
    expect(mineCountOf({}, 0, viewer)).toBe(0);
  });
});
