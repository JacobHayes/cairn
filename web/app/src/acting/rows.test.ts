// C9: grouping by container keeps the sort order within and between groups.
import { describe, expect, it } from "vitest";

import { byContainer } from "./rows.ts";
import type { NodeRow } from "./why.ts";

const row = (key: string, ancestors: string[]): NodeRow => ({
  key,
  ancestors,
  kind: "action",
  path: key,
  title: key,
  state: "todo",
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
