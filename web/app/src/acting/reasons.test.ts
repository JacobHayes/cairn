// C10, C11, D3: the one fact under a row's title: that it may not apply, how late or near its date
// is, what it needs, or the biggest reason it ranks where it does, in words and with its ink.
import { describe, expect, it } from "vitest";

import type { Ready } from "../detail/model.ts";
import { actingView } from "./acting.test-support.ts";
import { actionFact, factWords } from "./reasons.ts";
import type { NodeRow } from "./why.ts";

const RANK = { rank: 0, urgency: 0, late: 0, gravity_norm: 0, unlocks_norm: 0, adds: { urgency: 0, late: 0, gravity: 0, unlocks: 0 } };
/** What each term adds, as the engine serves it: the term times its constant. */
const adds = (extra: Partial<typeof RANK.adds>) => ({ ...RANK.adds, ...extra });

function row(key: string, extra: Partial<NodeRow> = {}): NodeRow {
  return { key, kind: "action", path: key, title: key, state: "todo", display_state: "ready", relevance: "relevant", gravity: 1, unlocks: 0, ...extra };
}

/** The acting view with `key`'s gravity and unlocks explanations set. */
function viewWith(key: string, from: { gravity?: number; unlocks?: { total: number; others: number } }): Ready {
  const view = actingView();
  const derived = view.derived.nodes[key];
  if (derived !== undefined) {
    derived.gravity_from = { total: from.gravity ?? 0, entries: [] };
    derived.unlocks_from = {
      total: from.unlocks?.total ?? 0,
      entries: Array.from({ length: from.unlocks?.others ?? 0 }, (_, at) => ({ node: `n_other${String(at)}`, score: 1, other_owner: true })),
    };
  }
  return view;
}

/** The acting view with the engine counting `key` five days late. */
function lateView(key: string): Ready {
  const view = viewWith(key, { gravity: 4 });
  const derived = view.derived.nodes[key];
  if (derived !== undefined) {
    derived.dates = { ...derived.dates, overdue_days: 5 };
  }
  return view;
}

/** The fact a row says, as words and ink, under `sort`. */
function said(view: Ready, each: NodeRow, sort?: Parameters<typeof actionFact>[2]) {
  const fact = actionFact(view, each, sort);
  return fact === undefined ? undefined : { words: factWords(fact, view.derived.today), tone: fact.tone };
}

// The acting view's today is 2026-10-06 and its rank constants weigh urgency 0.4, late 0.15,
// gravity 0.25 and unlocks 0.2.
describe("actionFact", () => {
  const cases: [string, NodeRow, Parameters<typeof viewWith>[1], { words: string; tone: "warn" | "bad" | undefined } | undefined][] = [
    ["the next part when the largest has nothing downstream to name", row("n_meet", { rank: { ...RANK, gravity_norm: 1, unlocks_norm: 0.9, adds: adds({ gravity: 0.25, unlocks: 0.18 }), rank: 0.43 } }), { gravity: 0, unlocks: { total: 2, others: 2 } }, { words: "Unlocks 2 others' work", tone: undefined }],
    ["a clearly larger weight over a date that is not near", row("n_meet", { slack_days: 12, due: "2026-10-30", rank: { ...RANK, urgency: 0.1, gravity_norm: 1, adds: adds({ urgency: 0.04, gravity: 0.25 }), rank: 0.29 } }), { gravity: 7 }, { words: "Gates 7 nodes", tone: undefined }],
    ["a conditional row", row("n_meet", { display_state: "conditional", slack_days: 0, rank: { ...RANK, urgency: 1, adds: adds({ urgency: 0.4 }), rank: 0.4 } }), {}, { words: "May not apply", tone: undefined }],
    ["a note the work requires, when no date presses", row("n_log", { slack_days: 9, due: "2026-10-30", rank: { ...RANK, urgency: 1, adds: adds({ urgency: 0.4 }), rank: 0.4 } }), {}, { words: "Note required", tone: undefined }],
    ["a row with nothing to say", row("n_meet"), {}, undefined],
  ];
  it.each(cases)("%s", (_name, each, from, expected) => {
    expect(said(viewWith(each.key, from), each)).toEqual(expected);
  });

  it("puts a near date ahead of a bigger weight and of what the row needs, in warning ink", () => {
    const view = viewWith("n_log", { gravity: 7 });
    expect(said(view, row("n_log", { slack_days: 5, due: "2026-10-12", rank: { ...RANK, urgency: 0.1, gravity_norm: 1, adds: adds({ urgency: 0.04, gravity: 0.25 }), rank: 0.29 } }))).toEqual({ words: "Due in 6 days", tone: "warn" });
    expect(said(view, row("n_log", { slack_days: 0, due: "2026-10-30" }))).toEqual({ words: "Start today", tone: "warn" });
  });

  it("says lateness before what the row needs, in error ink, whatever the list is sorted by", () => {
    const view = lateView("n_work");
    // The work really is missing its link: lateness still leads.
    const late = row("n_work", { overdue: true, due: "2026-10-01" });
    for (const sort of ["rank", "gravity", "unlocks", "due", "slack"] as const) {
      expect(said(view, late, sort)).toEqual({ words: "5 days late", tone: "bad" });
    }
    expect(said(view, row("n_work", { slack_days: -2 }))).toEqual({ words: "Should have started 2 days ago", tone: "bad" });
  });

  it("leaves what the row needs after a date that presses and before one that does not", () => {
    const view = actingView();
    expect(said(view, row("n_work", { slack_days: 9, due: "2026-10-30" }))?.words).toBe("Link required");
  });

  it("names a decision's date: late with how late, soon in warning ink, otherwise plain after what it needs", () => {
    const view = lateView("n_pick");
    const decide = (extra: Partial<NodeRow>) => said(view, row("n_pick", { kind: "decision", ...extra }));
    expect(decide({ due: "2026-10-01", overdue: true })).toEqual({ words: "Decide by Oct 1, 5 days late", tone: "bad" });
    expect(decide({ due: "2026-10-13" })).toEqual({ words: "Decide by Oct 13", tone: "warn" });
    expect(decide({ due: "2026-10-30" })).toEqual({ words: "Decide by Oct 30", tone: undefined });
  });

  it("leads with the signal the list is sorted by, over a near date but not over lateness", () => {
    const each = row("n_meet", { slack_days: 0, due: "2026-10-09", rank: { ...RANK, urgency: 1, gravity_norm: 0.2, adds: adds({ urgency: 0.4, gravity: 0.05 }), rank: 0.45 } });
    const view = viewWith("n_meet", { gravity: 4, unlocks: { total: 3, others: 0 } });
    expect(said(view, each, "gravity")).toEqual({ words: "Gates 4 nodes", tone: undefined });
    expect(said(view, each, "unlocks")).toEqual({ words: "Unlocks 3 nodes", tone: undefined });
    expect(said(view, each, "due")).toEqual({ words: "Start today", tone: "warn" });
  });
});
