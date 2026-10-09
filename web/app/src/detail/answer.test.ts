// B2: Save is enabled only for something new, and nothing is preselected.
import { describe, expect, it } from "vitest";

import { outgoing, type AnswerState } from "./answer.ts";

const fresh: AnswerState = { stored: undefined, storedReason: undefined, draft: undefined, reason: undefined, named: "" };
const decided: AnswerState = { ...fresh, stored: { boolean: true }, storedReason: "- cheaper" };

describe("what Save would send", () => {
  it("is nothing until something new is chosen, and a different answer starts with no reason", () => {
    expect(outgoing(fresh)).toBeUndefined();
    expect(outgoing({ ...decided, draft: { boolean: true } })).toBeUndefined();
    expect(outgoing({ ...fresh, draft: { boolean: false }, reason: "too slow" })).toEqual({ value: { boolean: false }, reason: "too slow" });
    expect(outgoing({ ...decided, draft: { boolean: false } })).toEqual({ value: { boolean: false }, reason: "" });
    expect(outgoing({ ...decided, reason: "- cheaper and faster" })).toEqual({ value: { boolean: true }, reason: "- cheaper and faster" });
  });
});
