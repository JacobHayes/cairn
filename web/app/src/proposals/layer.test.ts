import { describe, expect, it } from "vitest";

import type { Deployment } from "../data/host.ts";
import { entityDiff } from "./layer.ts";

const deployment = (entities: { key: string; name: string }[]): Deployment => ({ revision: 1, entities });

describe("entityDiff", () => {
  it("lists the rows by name, a created entity among them rather than last", () => {
    const before = deployment([{ key: "e_b", name: "Bea" }, { key: "e_d", name: "Dev" }]);
    const after = deployment([{ key: "e_b", name: "Bea" }, { key: "e_d", name: "Dev" }, { key: "e_a", name: "Ada" }]);
    const rows = entityDiff(before, after);
    expect(rows.map((row) => [row.entity.key, row.mark?.kind])).toEqual([["e_a", "added"], ["e_b", undefined], ["e_d", undefined]]);
  });
});
