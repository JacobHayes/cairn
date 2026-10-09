// The keyboard map's decisions: which row `j` and `k` move to.
import { describe, expect, it } from "vitest";

import { rowAfter } from "./keys.ts";

describe("j and k", () => {
  const keys = ["a", "b", "c"];

  it("move to the next and the previous row, and stop at the ends", () => {
    expect(rowAfter(keys, "a", 1)).toBe("b");
    expect(rowAfter(keys, "c", -1)).toBe("b");
    expect(rowAfter(keys, "c", 1)).toBeUndefined();
    expect(rowAfter(keys, "a", -1)).toBeUndefined();
  });

  it("start at the first row going down and the last going up when nothing is selected", () => {
    expect(rowAfter(keys, undefined, 1)).toBe("a");
    expect(rowAfter(keys, undefined, -1)).toBe("c");
    expect(rowAfter(keys, "gone", 1)).toBe("a");
    expect(rowAfter([], undefined, 1)).toBeUndefined();
  });
});
