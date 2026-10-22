import { describe, expect, it } from "vitest";

import { pickRange } from "./picks.ts";

describe("pickRange", () => {
  const order = ["a", "b", "c", "d", "e"];

  it("adds every row between the anchor and the row clicked, either way, keeping what was picked", () => {
    expect(new Set(pickRange(order, ["b", "e"], "b", "d"))).toEqual(new Set(["b", "c", "d", "e"]));
    expect(new Set(pickRange(order, ["d"], "d", "a"))).toEqual(new Set(["a", "b", "c", "d"]));
  });

  it("toggles the row alone without an anchor in the table", () => {
    expect(pickRange(order, ["a"], undefined, "c")).toEqual(["a", "c"]);
    expect(pickRange(order, ["a", "c"], "gone", "c")).toEqual(["a"]);
  });
});
