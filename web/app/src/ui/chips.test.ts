// The multi-value input's rules: what a typed or pasted list becomes, and what stays behind with its reason.
import { describe, expect, it } from "vitest";

import { EMAIL_RULES } from "../people/model.ts";
import { hasSeparator, takeChips } from "./chips.ts";

describe("taking values from text", () => {
  it("splits a pasted list at commas, semicolons, spaces and lines, normalized, in order", () => {
    const taken = takeChips([], " Ann@Example.org, bob@example.org;\ncy@example.org  ", EMAIL_RULES);
    expect(taken).toEqual({ values: ["ann@example.org", "bob@example.org", "cy@example.org"], text: "", problem: undefined });
  });

  it("keeps a failing value in the text with its reason and still takes the rest", () => {
    const taken = takeChips(["a@example.org"], "nope, b@example.org, a@example.org, c@@x", EMAIL_RULES);
    expect(taken.values).toEqual(["a@example.org", "b@example.org"]);
    expect(taken.text).toBe("nope, a@example.org, c@@x");
    expect(taken.problem).toContain("nope");
  });

  it("refuses a duplicate by saying it is already there, even when it differs by case", () => {
    const taken = takeChips(["a@example.org"], "A@Example.org", EMAIL_RULES);
    expect(taken.values).toEqual(["a@example.org"]);
    expect(taken.text).toBe("A@Example.org");
    expect(taken.problem).toContain("already");
  });

  it("refuses a value past the maximum", () => {
    const rules = { ...EMAIL_RULES, max: 2 };
    const taken = takeChips(["a@x.org", "b@x.org"], "c@x.org", rules);
    expect(taken.values).toHaveLength(2);
    expect(taken.text).toBe("c@x.org");
    expect(taken.problem).toBe(rules.maxProblem);
  });

  it("sees a separator only where one ends a value", () => {
    expect(hasSeparator("a@x.org")).toBe(false);
    expect(hasSeparator("a@x.org,")).toBe(true);
  });
});
