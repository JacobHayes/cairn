// The multi-value input's rules: what a typed or pasted list becomes, and what stays behind with its reason.
// The email rules are the engine's (the real engine answers here).
import type { ValueProblem } from "@cairn/wasm";
import { beforeAll, describe, expect, it } from "vitest";

import { seeded } from "../authoring/engine.test-support.ts";
import { emailRules } from "../people/model.ts";
import { hasSeparator, takeChips, type ChipRules } from "./chips.ts";

let rules: ChipRules;
beforeAll(async () => {
  const host = await seeded();
  rules = emailRules({ valueProblems: (checks) => Promise.resolve(JSON.parse(host.engine.valueProblemsText(checks)) as ValueProblem[]) });
});

describe("taking values from text", () => {
  it("splits a pasted list at commas, semicolons, spaces and lines, normalized, in order", async () => {
    const taken = await takeChips([], " Ann@Example.org, bob@example.org;\ncy@example.org  ", rules);
    expect(taken).toEqual({ values: ["ann@example.org", "bob@example.org", "cy@example.org"], text: "", problem: undefined });
  });

  it("keeps a failing value in the text with its reason and still takes the rest", async () => {
    const taken = await takeChips(["a@example.org"], "nope, b@example.org, a@example.org, c@@x", rules);
    expect(taken.values).toEqual(["a@example.org", "b@example.org"]);
    expect(taken.text).toBe("nope, a@example.org, c@@x");
    expect(taken.problem).toContain("nope");
  });

  it("refuses a duplicate, even when it differs by case", async () => {
    const taken = await takeChips(["a@example.org"], "A@Example.org", rules);
    expect(taken.values).toEqual(["a@example.org"]);
    expect(taken.text).toBe("A@Example.org");
    expect(taken.problem).toContain("a@example.org");
  });

  it("refuses a value past the most an identity holds", async () => {
    const held = Array.from({ length: 16 }, (_, at) => `p${String(at)}@x.org`);
    const taken = await takeChips(held, "one-more@x.org", rules);
    expect(taken.values).toEqual(held);
    expect(taken.text).toBe("one-more@x.org");
    expect(taken.problem).toContain("16");
  });

  it("refuses an email past the engine's byte limit", async () => {
    const taken = await takeChips([], `${"x".repeat(250)}@e.org`, rules);
    expect(taken.values).toEqual([]);
  });

  it("sees a separator only where one ends a value", () => {
    expect(hasSeparator("a@x.org")).toBe(false);
    expect(hasSeparator("a@x.org,")).toBe(true);
  });
});
