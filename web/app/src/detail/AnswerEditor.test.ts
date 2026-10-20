// B3, E6: an identity answer picks existing identities or names a new one, made in the same patch
// as the answer that names it.
import { describe, expect, it } from "vitest";

import { answerMutations } from "./AnswerEditor.tsx";

const key = () => "who_new";

describe("an identity answer with a new identity (B3)", () => {
  it("makes the identity first and answers with it, or adds it to a list answer", () => {
    expect(answerMutations("n_who", { identity: "" }, " Ann ", "", key)).toEqual([
      { op: "create_identity", identity: { key: "who_new", name: "Ann" } },
      { op: "answer", decision: "n_who", value: { identity: "who_new" } },
    ]);
    expect(answerMutations("n_who", { identity_list: ["who_a"] }, "Ann", "", key)[1]).toEqual({ op: "answer", decision: "n_who", value: { identity_list: ["who_a", "who_new"] } });
  });

  it("answers alone when no new identity is named, or the answer is not an identity", () => {
    expect(answerMutations("n_who", { identity: "who_a" }, "  ", "", key)).toEqual([{ op: "answer", decision: "n_who", value: { identity: "who_a" } }]);
    expect(answerMutations("n_ok", { boolean: true }, "Ann", "", key)).toEqual([{ op: "answer", decision: "n_ok", value: { boolean: true } }]);
  });
});

describe("an answer's rationale (B2)", () => {
  it("rides on the answer, also one that names a new identity, trimmed, and is left off when blank so a revision gives none", () => {
    expect(answerMutations("n_ok", { boolean: true }, "", "- cheaper\n- [quote](https://example.org)\n", key)).toEqual([
      { op: "answer", decision: "n_ok", value: { boolean: true }, rationale: "- cheaper\n- [quote](https://example.org)" },
    ]);
    expect(answerMutations("n_ok", { boolean: false }, "", " \n ", key)).toEqual([{ op: "answer", decision: "n_ok", value: { boolean: false } }]);
    expect(answerMutations("n_who", { identity: "" }, "Ann", "Knows the system", key)[1]).toEqual({ op: "answer", decision: "n_who", value: { identity: "who_new" }, rationale: "Knows the system" });
  });
});
