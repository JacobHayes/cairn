// C9, C10: the due line a phone's list row says under its title, where the Due column is dropped: a
// late row says how late once, as its fact, and not again as a due line; a row whose fact is
// something else keeps its due line, and a row that is not open has none.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { dueWords } from "../status/when.ts";
import { actingView } from "./acting.test-support.ts";
import { factOf } from "./reasons.ts";
import { PhoneDue } from "./ListCells.tsx";
import type { Line } from "./listModel.ts";
import type { NodeRow } from "./why.ts";

const RANK = { rank: 0, urgency: 0, late: 0, gravity_norm: 0, unlocks_norm: 0, adds: { urgency: 0, late: 0, gravity: 0, unlocks: 0 } };

const row = (extra: Partial<NodeRow>): NodeRow => ({ key: "n_work", kind: "deliverable", path: "n_work", title: "Write it", state: "todo", display_state: "ready", relevance: "relevant", gravity: 1, unlocks: 0, ...extra });

const LATE_DAYS = 5;

/** The acting view with `each`'s due date, and the days the engine counts it late when the row is overdue. */
function viewWith(each: NodeRow) {
  const view = actingView();
  const derived = view.derived.nodes[each.key];
  if (derived !== undefined && each.due != null) {
    derived.dates = { ...derived.dates, due: { date: each.due, chain: { constraints: [], fixed: [] } }, ...(each.overdue === true ? { overdue_days: LATE_DAYS } : {}) };
  }
  return view;
}

/** The due line under `each`'s title with Frontier on, whose fact it dedupes against; empty when there is none. */
function dueLine(each: NodeRow): string {
  const view = viewWith(each);
  const line: Line = { key: each.key, depth: 0, row: each, container: false, foldable: false, open: false, guides: [] };
  return renderToStaticMarkup(<PhoneDue view={view} line={line} fact={factOf(view, each, "rank")?.words} />).replace(/<[^>]*>/g, "");
}

describe("a phone's due line under a row's title", () => {
  it("is not said again when the row's fact is how late it is", () => {
    const late = row({ overdue: true, due: "2026-10-01", rank: { ...RANK, late: 1, adds: { ...RANK.adds, late: 0.15 }, rank: 0.15 } });
    const view = viewWith(late);
    expect(factOf(view, late, "rank")?.words).toBe(dueWords("2026-10-01", view.derived.today, false, LATE_DAYS));
    expect(dueLine(late)).toBe("");
  });

  it("is kept under a row whose fact is something else", () => {
    const gated = row({ due: "2026-10-30", slack_days: 12, rank: { ...RANK, urgency: 0.1, gravity_norm: 1, adds: { ...RANK.adds, urgency: 0.04, gravity: 0.25 }, rank: 0.29 } });
    expect(dueLine(gated)).toContain(dueWords("2026-10-30", actingView().derived.today, false, undefined));
  });
});
