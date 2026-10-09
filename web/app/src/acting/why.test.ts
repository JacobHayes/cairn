// C10: why an item ranks where it does: the blend's parts add up to the rank, largest first,
// and a list sorted by one signal reads that signal.
import { describe, expect, it } from "vitest";

import { rankParts, signalOf, type NodeRow, type RankConstants } from "./why.ts";

const constants: RankConstants = { urgency: 0.4, late: 0.15, gravity: 0.25, leverage: 0.2, horizon_days: 14, undecided_discount: 0.5, other_owner_factor: 2 };

const row: NodeRow = {
  key: "n_a",
  kind: "action",
  path: "a",
  title: "A",
  state: "todo",
  display_state: "ready",
  relevance: "relevant",
  gravity: 6,
  leverage: 2,
  slack_days: 7,
  due: "2026-10-20",
  rank: { rank: 0.4, urgency: 0.5, late: 0, gravity_norm: 0.6, leverage_norm: 0.25 },
};

describe("rankParts (Priority)", () => {
  const parts = rankParts(row.rank ?? { rank: 0, urgency: 0, late: 0, gravity_norm: 0, leverage_norm: 0 }, constants);

  it("adds up to the rank", () => {
    const sum = parts.reduce((total, part) => total + part.adds, 0);
    expect(sum).toBeCloseTo(row.rank?.rank ?? 0, 9);
  });

  it("puts the largest part first and leaves out what adds nothing", () => {
    expect(parts.map((part) => part.signal)).toEqual(["urgency", "gravity", "leverage"]);
  });
});

describe("signalOf (C10: re-sort by one signal)", () => {
  const cases: [Parameters<typeof signalOf>[1], number | undefined, number | string | undefined][] = [
    ["rank", undefined, 0.4],
    ["slack", undefined, 7],
    ["gravity", undefined, 6],
    ["leverage", undefined, 2],
    ["due", undefined, "2026-10-20"],
    ["effort", 3, 2],
    ["effort", 0, undefined],
    ["effort", undefined, undefined],
  ];
  it.each(cases)("%s with estimate %s", (sort, estimate, expected) => {
    expect(signalOf(row, sort, estimate)).toEqual(expected);
  });

  it("has no slack for a row with no deadline", () => {
    expect(signalOf({ ...row, slack_days: null }, "slack")).toBeUndefined();
  });
});
