import { describe, expect, it } from "vitest";

import { namedList, plural } from "./words.ts";

describe("namedList", () => {
  it("joins what fits and counts the rest, never leaving one over", () => {
    const names = ["A", "B", "C", "D", "E"];
    expect(namedList([])).toBe("");
    for (const length of [1, 2, 3]) {
      const said = namedList(names.slice(0, length));
      names.slice(0, length).forEach((name) => {
        expect(said).toContain(name);
      });
      expect(said).not.toMatch(/\d/);
    }
    expect(namedList(["A", "B"])).toBe("A and B");
    const four = namedList(names.slice(0, 4));
    expect(four).toMatch(/\b2 more\b/);
    expect(four).not.toContain("D");
    expect(namedList(names)).toMatch(/\b3 more\b/);
  });
});

describe("plural", () => {
  it("takes the singular only for one", () => {
    expect(plural(1, "node")).toBe("1 node");
    expect(plural(0, "node")).toBe("0 nodes");
    expect(plural(2, "entry", "entries")).toBe("2 entries");
  });
});
