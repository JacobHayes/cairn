// A small derived journey for node detail's unit tests, written as the derive worker answers
// it: a stage holding a report that requires an artifact and is pinned, a meeting milestone an
// answered date decision feeds (E3), an open decision, and a findings deliverable the report requires.
import type { Mutation } from "../authoring/graph.ts";
import { builtJourney, loadedEngine } from "../authoring/engine.test-support.ts";
import type { Bound } from "./explain.ts";
import type { Ready } from "./model.ts";

await loadedEngine();

const today = "2026-10-06";

/** The report's due date: fixed by its own pin, with no constraint between. */
const pinnedDue: Bound = {
  date: "2026-11-02",
  chain: {
    constraints: [],
    fixed: [{ instant: "n_report.finish", date: "2026-11-02", fixed_by: "pin" }],
  },
};

/** The findings' due date: derived through the report's requirement on it, from the pin. */
const derivedDue: Bound = {
  date: "2026-10-30",
  chain: {
    constraints: [
      {
        before: "n_findings.finish",
        after: "n_report.start",
        days: 0,
        source: "dependency:n_report:n_findings:explicit",
      },
      {
        before: "n_report.start",
        after: "n_report.finish",
        days: 3,
        source: "estimate:n_report",
      },
    ],
    fixed: [{ instant: "n_report.finish", date: "2026-11-02", fixed_by: "pin" }],
  },
};

/** The meeting's due: fixed by its feeding decision's answer, shown as a pin (E3). */
const meetingDue: Bound = {
  date: "2026-11-20",
  chain: {
    constraints: [],
    fixed: [{ instant: "n_meeting.finish", date: "2026-11-20", fixed_by: "pin" }],
  },
};

const relevant = { relevance: { value: "relevant" as const }, snoozable: true, open: true, gravity: 1, unlocks: 0 };
const none = { entries: [], total: 0 };

/** The derive's per-node values. */
function derivedNodes(): Ready["derived"]["nodes"] {
  return {
    n_stage: {
      ...relevant,
      skip_count: 2,
      moves: ["insert_route", "mark_done", "skip"],
      display_state: "active",
      actionable: false,
      dates: {},
      gravity_from: none,
      unlocks_from: none,
      waits_on: none,
      blocked_by: [{ node: "n_findings", via: "containment" }, { node: "n_report", via: "containment" }],
      progress: { done: 0, total: 2, overdue: 0, decisions_needed: 0 },
    },
    n_report: {
      ...relevant,
      moves: ["complete", "insert_route", "skip", "stop"],
      display_state: "active",
      actionable: false,
      blocked_by: [{ node: "n_findings", via: "explicit" }],
      waits_on: { entries: [{ node: "n_findings", via: "explicit" }], total: 1 },
      dates: { due: pinnedDue, slack_days: 24 },
      gravity: 5,
      gravity_from: { entries: [{ node: "n_meeting", score: 4 }], total: 1 },
      unlocks_from: none,
    },
    n_findings: {
      ...relevant,
      moves: ["complete", "insert_route", "skip", "start"],
      display_state: "ready",
      actionable: true,
      dates: { due: derivedDue },
      waits_on: none,
      gravity_from: none,
      unlocks: 2,
      unlocks_from: { entries: [{ node: "n_report", score: 1.5, other_owner: true }], total: 1 },
    },
    n_meeting: { ...relevant, moves: ["complete", "skip"], display_state: "scheduled", actionable: true, waits_on: none, dates: { due: meetingDue, effective_date: { date: "2026-11-20", origin: "pin" } }, gravity_from: none, unlocks_from: none },
    n_when: { ...relevant, snoozable: false, open: false, answer: { date: "2026-11-20" }, moves: ["answer", "reopen"], display_state: "done", actionable: false, waits_on: none, dates: {}, gravity_from: none, unlocks_from: none },
    n_pick: { ...relevant, moves: ["answer", "skip"], display_state: "ready", actionable: true, waits_on: none, dates: {}, gravity_from: none, unlocks_from: none },
  };
}

/**
 * A journey the engine derives with a checklist on Findings (items a, b and c, all required
 * before done) and a plain Report; `ticked` of the items are ticked.
 */
export function checklistView(ticked: string[] = ["b"]): Ready {
  return builtJourney("j_checklist", "Checklist journey", [
    { op: "add_node", node: { key: "n_findings", id: "findings", kind: "deliverable", title: "Findings", requires_checklist: true, checklist: [{ id: "a", title: "Rollback plan written" }, { id: "b", title: "Support team briefed" }, { id: "c", title: "Security sign-off" }] } },
    { op: "add_node", node: { key: "n_report", id: "report", kind: "action", title: "Report" } },
    ...ticked.map((item) => ({ op: "set_ticked", node: "n_findings", item, ticked: true })),
  ] as Mutation[]);
}

export function testView(): Ready {
  const view: Ready = {
    status: "ready",
    key: { journey: "j_test", revision: 7, deployment_revision: 2, today, now: `${today}T12:00:00Z` },
    inputs: {
      today,
      now: `${today}T12:00:00Z`,
      timezone: "UTC",
      deployment: { revision: 2, identities: [{ key: "who_one", name: "Person One" }, { key: "who_two", name: "Person Two" }] },
    },
    journey: {
      header: { id: "j_test", name: "Test journey", status: "active", created_at: "2026-10-01T00:00:00Z", created_on: "2026-10-01" },
      revision: 7,
      graph: {
        nodes: [
          { key: "n_stage", id: "stage", kind: "group", title: "Stage" },
          { key: "n_report", id: "report", parent: "n_stage", kind: "deliverable", title: "Report", requires_artifact: true, requires: ["n_findings"], estimate: 3 },
          { key: "n_findings", id: "findings", parent: "n_stage", kind: "deliverable", title: "Findings" },
          { key: "n_meeting", id: "meeting", kind: "milestone", title: "Meeting" },
          { key: "n_when", id: "when", kind: "decision", title: "Meeting date", prompt: "When?", answer_type: "date", feeds_milestone: "n_meeting" },
          { key: "n_pick", id: "pick", kind: "decision", title: "Pick one", prompt: "Which?", answer_type: "boolean" },
        ],
        state: {
          nodes: { n_report: { state: "active", started_on: "2026-10-05" } },
          pins: { n_report: "2026-11-02" },
          answers: { n_when: { date: "2026-11-20" } },
          annotations: [
            { body: { key: "a_link", node: "n_report", artifact: "https://example.org/report" }, created_by: "u_one", created_at: "2026-10-05T10:00:00Z" },
            { body: { key: "a_other", node: "n_findings", note: "Elsewhere." }, created_by: "u_one", created_at: "2026-10-05T10:00:00Z" },
          ],
        },
      },
    },
    derived: {
      today,
      now: `${today}T12:00:00Z`,
      frontier: ["n_findings"],
      acting_frontier: ["n_findings"],
      current_stages: ["n_stage"],
      nodes: derivedNodes(),
    },
  };
  return view;
}
