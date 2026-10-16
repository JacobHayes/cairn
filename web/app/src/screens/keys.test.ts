// The keyboard map's decisions: which row `j` and `k` move to, and which step `1` to `4` picks.
import { describe, expect, it } from "vitest";

import { rowAfter, stepForKey } from "./keys.ts";

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
  it("pick Stages to All, only the steps the journey offers", () => {
    const every = ["stages", "decisions", "work", "all"] as const;
    expect(["1", "2", "3", "4"].map((key) => stepForKey(key, every))).toEqual(every);
    expect(stepForKey("1", ["decisions", "work", "all"])).toBeUndefined();
    expect(stepForKey("5", every)).toBeUndefined();
    expect(stepForKey("d", every)).toBeUndefined();
  });
});
