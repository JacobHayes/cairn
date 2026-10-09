// A small derived journey for node detail's unit tests, written as the derive worker answers
// it: a stage holding a report that requires an artifact and is pinned, a meeting milestone a
// date decision feeds (E3), and a findings deliverable the report requires.
import type { Bound } from "./explain.ts";
import type { Ready } from "./model.ts";

const today = "2026-10-06";

/** The report's due date: fixed by its own pin, with no constraint between. */
const pinnedDue: Bound = {
  date: "2026-11-02",
  chain: {
    constraints: [],
    fixed: [{ instant: { node: { node: "n_report", point: "finish" } }, date: "2026-11-02", fixed_by: "pin" }],
  },
};

/** The findings' due date: derived through the report's requirement on it, from the pin. */
const derivedDue: Bound = {
  date: "2026-10-30",
  chain: {
    constraints: [
      {
        before: { node: { node: "n_findings", point: "finish" } },
        after: { node: { node: "n_report", point: "start" } },
        offset_days: 0,
        source: { dependency: { node: "n_report", requires: "n_findings", via: "explicit" } },
      },
      {
        before: { node: { node: "n_report", point: "start" } },
        after: { node: { node: "n_report", point: "finish" } },
        offset_days: 3,
        source: { estimate: { node: "n_report" } },
      },
    ],
    fixed: [{ instant: { node: { node: "n_report", point: "finish" } }, date: "2026-11-02", fixed_by: "pin" }],
  },
};

/** The meeting's due: fixed by its feeding decision's answer, shown as a pin (E3). */
const meetingDue: Bound = {
  date: "2026-11-20",
  chain: {
    constraints: [],
    fixed: [{ instant: { node: { node: "n_meeting", point: "finish" } }, date: "2026-11-20", fixed_by: "pin" }],
  },
};

const relevant = { relevance: { value: "relevant" as const }, gravity: 1, leverage: 0 };
const none = { entries: [], total: 0 };

export function testView(): Ready {
  const view: Ready = {
    status: "ready",
    key: { journey: "j_test", revision: 7, deployment_revision: 2, today },
    inputs: {
      today,
      timezone: "UTC",
      deployment: { revision: 2, entities: [{ key: "e_one", name: "Person One" }, { key: "e_two", name: "Person Two" }] },
      rank: { urgency: 0.4, late: 0.15, gravity: 0.25, leverage: 0.2, horizon_days: 14, undecided_discount: 0.5, other_owner_factor: 2 },
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
        ],
        state: {
          nodes: { n_report: { state: "active", provenance: "from_route", started_on: "2026-10-05" } },
          pins: { n_report: "2026-11-02" },
          answers: { n_when: { date: "2026-11-20" } },
          annotations: [
            { body: { key: "a_link", node: "n_report", artifact: "https://example.org/report" }, created_by: "u_one", created_at: "2026-10-05T10:00:00Z" },
            { body: { key: "a_other", node: "n_findings", note: "Elsewhere." }, created_by: "u_one", created_at: "2026-10-05T10:00:00Z" },
          ],
          local_edits: { n_report: [{ field: "title" }] },
        },
      },
    },
    derived: {
      today,
      frontier: ["n_findings"],
      acting_frontier: ["n_findings"],
      nodes: {
        n_stage: { ...relevant, display_state: "active", actionable: false, dates: {}, gravity_from: none, leverage_from: none, blocked_by: [{ node: "n_findings", via: "containment" }] },
        n_report: {
          ...relevant,
          display_state: "active",
          actionable: false,
          blocked_by: [{ node: "n_findings", via: "explicit" }],
          dates: { due: pinnedDue, slack_days: 24 },
          gravity: 5,
          gravity_from: { entries: [{ node: "n_meeting", score: 4 }], total: 1 },
          leverage_from: none,
        },
        n_findings: {
          ...relevant,
          display_state: "ready",
          actionable: true,
          dates: { due: derivedDue },
          gravity_from: none,
          leverage: 2,
          leverage_from: { entries: [{ node: "n_report", score: 1.5, other_owner: true }], total: 1 },
        },
        n_meeting: { ...relevant, display_state: "ready", actionable: false, dates: { due: meetingDue, effective_date: { date: "2026-11-20", origin: "pin" } }, gravity_from: none, leverage_from: none },
        n_when: { ...relevant, display_state: "ready", actionable: false, dates: {}, gravity_from: none, leverage_from: none },
      },
    },
  };
  return view;
}
