// C9: NEXT narrows by flags.
import { describe, expect, it } from "vitest";

import { withFlags } from "./rows.ts";
import type { NodeRow } from "./why.ts";

const row = (key: string): NodeRow => ({
  key,
  ancestors: [],
  kind: "action",
  path: key,
  title: key,
  state: "todo",
  display_state: "ready",
  relevance: "relevant",
  gravity: 1,
  unlocks: 0,
});

describe("withFlags", () => {
  it("keeps the rows that have every flag asked for, and every row when none is", () => {
    const rows = [{ ...row("n_a"), overdue: true }, { ...row("n_b"), overdue: true, stale: true }, row("n_c")];
    expect(withFlags(rows, []).map((each) => each.key)).toEqual(["n_a", "n_b", "n_c"]);
    expect(withFlags(rows, ["overdue"]).map((each) => each.key)).toEqual(["n_a", "n_b"]);
    expect(withFlags(rows, ["overdue", "stale"]).map((each) => each.key)).toEqual(["n_b"]);
  });
});
