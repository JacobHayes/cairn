// C14, B2: a proposal's answer says the reason it carries, so a reviewer sees what it records.
import { describe, expect, it } from "vitest";

import { mutationWords, namesOf } from "./words.ts";

describe("mutationWords", () => {
  it("adds an answer's reason, its first line only, and nothing when it has none", () => {
    const names = namesOf([], []);
    const answer = { op: "answer", decision: "n_ok", value: { boolean: true } } as const;
    expect(mutationWords(answer, names)).toBe('Answer "n_ok": true');
    expect(mutationWords({ ...answer, rationale: "Cheaper.\n\n- and faster" }, names)).toBe('Answer "n_ok": true, because "Cheaper."');
  });
});
