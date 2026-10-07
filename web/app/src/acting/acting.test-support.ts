// A small derived journey for the acting surfaces' unit tests, written as the derive worker
// answers it: a meeting milestone and an unassigned decision on the acting frontier; a
// deliverable that requires an artifact; a placeholder still to break down; a decision
// snoozed until the deliverable is done; a later decision that requires the meeting; a stage
// the meeting opens, holding a decision; a finished action; and a decision no longer in scope.
import type { Ready } from "../detail/model.ts";

const today = "2026-10-06";
const none = { entries: [], total: 0 };
const base = { gravity_from: none, leverage_from: none, actionable: false, gravity: 1, leverage: 0, dates: {} };
const relevant = { relevance: { value: "relevant" as const } };
const start = (date: string) => ({ earliest_start: { date, chain: { constraints: [], fixed: [] } } });

export function actingView(): Ready {
  return {
    status: "ready",
    key: { journey: "j_act", revision: 5, deployment_revision: 1, today },
    inputs: {
      today,
      timezone: "UTC",
      deployment: { revision: 1, entities: [{ key: "e_one", name: "Person One" }] },
      rank: { urgency: 0.4, late: 0.15, gravity: 0.25, leverage: 0.2, horizon_days: 14, undecided_discount: 0.5, other_owner_factor: 2 },
    },
    journey: {
      header: { id: "j_act", name: "Acting journey", status: "active", created_at: "2026-10-01T00:00:00Z", created_on: "2026-10-01" },
      revision: 5,
      graph: {
        nodes: [
          { key: "n_meet", id: "meet", kind: "milestone", title: "Meeting" },
          { key: "n_pick", id: "pick", kind: "decision", title: "Pick one", answer_type: "boolean" },
          { key: "n_work", id: "work", kind: "deliverable", title: "Write it", requires_artifact: true },
          { key: "n_hold", id: "hold", kind: "action", title: "Break me down", placeholder: true },
          { key: "n_wait", id: "wait", kind: "decision", title: "Wait for it", answer_type: "boolean" },
          { key: "n_later", id: "later", kind: "decision", title: "Decide later", answer_type: "boolean", requires: ["n_meet"] },
          { key: "n_stage", id: "stage", kind: "group", title: "Stage", opens_at: "n_meet" },
          { key: "n_inner", id: "inner", parent: "n_stage", kind: "decision", title: "Inside the stage", answer_type: "boolean" },
          { key: "n_done", id: "done", kind: "action", title: "Finished" },
          { key: "n_gone", id: "gone", kind: "decision", title: "Out of scope", answer_type: "boolean" },
        ],
        state: {
          nodes: {
            n_done: { state: "done", provenance: "from_route" },
          },
          snoozes: { n_wait: { node: "n_work" } },
        },
      },
    },
    derived: {
      today,
      frontier: ["n_meet", "n_pick", "n_work", "n_hold", "n_wait"],
      acting_frontier: ["n_meet", "n_pick", "n_work", "n_hold"],
      nodes: {
        n_meet: { ...base, ...relevant, actionable: true },
        n_pick: { ...base, ...relevant, actionable: true, unassigned: true },
        n_work: { ...base, ...relevant, actionable: true },
        n_hold: { ...base, ...relevant, actionable: true, needs_breakdown: true },
        n_wait: { ...base, ...relevant, actionable: true, snoozed: { node: "n_work" }, dates: start("2026-10-06") },
        n_later: { ...base, ...relevant, blocked_by: [{ node: "n_meet", via: "explicit" }], dates: start("2026-10-20") },
        n_stage: {
          ...base,
          ...relevant,
          blocked_by: [
            { node: "n_meet", via: { stage_opening: { group: "n_stage" } } },
            { node: "n_inner", via: "containment" },
          ],
        },
        n_inner: { ...base, ...relevant, blocked_through: ["n_stage"], dates: start("2026-10-13") },
        n_done: { ...base, ...relevant },
        n_gone: { ...base, relevance: { value: "not_relevant" } },
      },
    },
  };
}
