// B3, E6: an entity answer picks existing entities or names a new one, made in the same patch
// as the answer that names it.
import { describe, expect, it } from "vitest";

import { answerMutations } from "./AnswerEditor.tsx";

const key = () => "e_new";

describe("an entity answer with a new entity (B3)", () => {
  it("makes the entity first and answers with it, or adds it to a list answer", () => {
    expect(answerMutations("n_who", { entity: "" }, " Ann ", "", key)).toEqual([
      { op: "create_entity", entity: { key: "e_new", name: "Ann" } },
      { op: "answer", decision: "n_who", value: { entity: "e_new" } },
    ]);
    expect(answerMutations("n_who", { entity_list: ["e_a"] }, "Ann", "", key)[1]).toEqual({ op: "answer", decision: "n_who", value: { entity_list: ["e_a", "e_new"] } });
  });

  it("answers alone when no new entity is named, or the answer is not an entity", () => {
    expect(answerMutations("n_who", { entity: "e_a" }, "  ", "", key)).toEqual([{ op: "answer", decision: "n_who", value: { entity: "e_a" } }]);
    expect(answerMutations("n_ok", { boolean: true }, "Ann", "", key)).toEqual([{ op: "answer", decision: "n_ok", value: { boolean: true } }]);
  });
});

describe("an answer's rationale (B2)", () => {
  it("rides on the answer, trimmed, and is left off when blank so a revision gives none", () => {
    expect(answerMutations("n_ok", { boolean: true }, "", "- cheaper\n- [quote](https://example.org)\n", key)).toEqual([
      { op: "answer", decision: "n_ok", value: { boolean: true }, rationale: "- cheaper\n- [quote](https://example.org)" },
    ]);
    expect(answerMutations("n_ok", { boolean: false }, "", " \n ", key)).toEqual([{ op: "answer", decision: "n_ok", value: { boolean: false } }]);
  });

  it("goes with the answer that names a new entity", () => {
    expect(answerMutations("n_who", { entity: "" }, "Ann", "Knows the system", key)[1]).toEqual({ op: "answer", decision: "n_who", value: { entity: "e_new" }, rationale: "Knows the system" });
  });
});
