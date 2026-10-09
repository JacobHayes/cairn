// C10: the one fact under a row's title is the biggest reason it ranks where it does, in words.
import { describe, expect, it } from "vitest";

import type { Ready } from "../detail/model.ts";
import { actingView } from "./acting.test-support.ts";
import { reasonOf } from "./reasons.ts";
import type { NodeRow } from "./why.ts";

const RANK = { rank: 0, urgency: 0, late: 0, gravity_norm: 0, leverage_norm: 0 };

function row(key: string, extra: Partial<NodeRow> = {}): NodeRow {
  return { key, kind: "action", path: key, title: key, state: "todo", display_state: "ready", relevance: "relevant", gravity: 1, leverage: 0, ...extra };
}

/** The acting view with `key`'s gravity and leverage explanations set. */
function viewWith(key: string, from: { gravity?: number; leverage?: { total: number; others: number } }): Ready {
  const view = actingView();
  const derived = view.derived.nodes[key];
  if (derived !== undefined) {
    derived.gravity_from = { total: from.gravity ?? 0, entries: [] };
    derived.leverage_from = {
      total: from.leverage?.total ?? 0,
      entries: Array.from({ length: from.leverage?.others ?? 0 }, (_, at) => ({ node: `n_other${String(at)}`, score: 1, other_owner: true })),
    };
  }
  return view;
}

// The acting view's today is 2026-10-06 and its rank constants weigh urgency 0.4, late 0.15,
// gravity 0.25 and leverage 0.2.
describe("reasonOf", () => {
  const cases: [string, NodeRow, Parameters<typeof viewWith>[1], string | undefined][] = [
    ["the next part when the largest has nothing downstream to name", row("n_meet", { rank: { ...RANK, gravity_norm: 1, leverage_norm: 0.9, rank: 0.43 } }), { gravity: 0, leverage: { total: 2, others: 2 } }, "Unblocks 2 others' work"],
    ["a date reason when it is close to the largest", row("n_meet", { slack_days: 0, due: "2026-10-09", rank: { ...RANK, urgency: 0.5, gravity_norm: 0.85, rank: 0.4125 } }), { gravity: 7 }, "Start today"],
    ["a clearly larger weight over a small date reason", row("n_meet", { slack_days: 5, due: "2026-10-12", rank: { ...RANK, urgency: 0.1, gravity_norm: 1, rank: 0.29 } }), { gravity: 7 }, "Gates 7 nodes"],
    ["a conditional row", row("n_meet", { display_state: "conditional", slack_days: 0, rank: { ...RANK, urgency: 1, rank: 0.4 } }), {}, "May not apply"],
    ["a note the work requires", row("n_log", { slack_days: 0, rank: { ...RANK, urgency: 1, rank: 0.4 } }), {}, "Note required"],
  ];
  it.each(cases)("%s", (_name, each, from, expected) => {
    expect(reasonOf(viewWith(each.key, from), each)).toBe(expected);
  });

  it("leads with the signal the list is sorted by", () => {
    const each = row("n_meet", { slack_days: 0, due: "2026-10-09", rank: { ...RANK, urgency: 1, gravity_norm: 0.2, rank: 0.45 } });
    const view = viewWith("n_meet", { gravity: 4, leverage: { total: 3, others: 0 } });
    expect(reasonOf(view, each, "gravity")).toBe("Gates 4 nodes");
  });
});
