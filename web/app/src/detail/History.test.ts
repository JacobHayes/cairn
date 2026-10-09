// B2, J1: a node's history shows each answer with the rationale it was written with.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { answersWritten } from "./History.tsx";

type PatchEvents = Schema<"PatchEvents">;

function answered(decision: string, value: Schema<"AnswerValue">, rationale?: string): PatchEvents {
  const record = { answer: { decision, value, ...(rationale === undefined ? {} : { rationale }) } };
  return {
    patch_id: "p_one",
    events: [
      {
        patch_id: "p_one",
        ordinal: 0,
        log: { journey: "j_test" },
        event_type: "answer_set",
        actor: { user: "u_one" },
        subject: { node: decision },
        at: "2026-10-06T12:00:00Z",
        delta: [{ put: { graph: { graph: { journey: "j_test" }, record } } }],
      },
    ],
  };
}

describe("answersWritten", () => {
  it("gives each answer its own reason, or none", () => {
    const patches = [answered("n_ok", { boolean: true }, "- cheaper"), answered("n_ok", { boolean: false }), answered("n_other", { boolean: true }, "elsewhere")];
    expect(patches.flatMap((patch) => answersWritten(patch, "n_ok"))).toEqual([
      { value: { boolean: true }, rationale: "- cheaper" },
      { value: { boolean: false }, rationale: undefined },
    ]);
  });
});
