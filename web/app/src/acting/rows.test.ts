// C9: grouping by container keeps the sort order within and between groups; NEXT narrows by flags.
import { describe, expect, it } from "vitest";

import { byContainer, withFlags } from "./rows.ts";
import type { NodeRow } from "./why.ts";

const row = (key: string, ancestors: string[]): NodeRow => ({
  key,
  ancestors,
  kind: "action",
  path: key,
  title: key,
  state: "todo",
  display_state: "ready",
  relevance: "relevant",
  gravity: 1,
  leverage: 0,
});

describe("byContainer", () => {
  it("groups rows under their nearest container, each group where its first row falls", () => {
    const rows = [row("n_a", ["n_setup"]), row("n_b", []), row("n_c", ["n_setup", "n_plan"]), row("n_d", ["n_setup"])];
    expect(byContainer(rows).map((group) => [group.container, group.rows.map((each) => each.key)])).toEqual([
      ["n_setup", ["n_a", "n_d"]],
      [undefined, ["n_b"]],
      ["n_plan", ["n_c"]],
    ]);
  });

  it("keeps each group's place in the tree", () => {
    expect(byContainer([row("n_c", ["n_setup", "n_plan"])])[0]?.path).toEqual(["n_setup", "n_plan"]);
  });
});

describe("withFlags", () => {
  it("keeps the rows that have every flag asked for, and every row when none is", () => {
    const rows = [{ ...row("n_a", []), overdue: true }, { ...row("n_b", []), overdue: true, stale: true }, row("n_c", [])];
    expect(withFlags(rows, []).map((each) => each.key)).toEqual(["n_a", "n_b", "n_c"]);
    expect(withFlags(rows, ["overdue"]).map((each) => each.key)).toEqual(["n_a", "n_b"]);
    expect(withFlags(rows, ["overdue", "stale"]).map((each) => each.key)).toEqual(["n_b"]);
  });
});
