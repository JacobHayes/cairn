// A small derived journey and its canvas levels for the canvas's unit tests, written as the
// derive worker answers them: a kickoff milestone (reached) opening a build stage, whose build
// deliverable is active while blocked by an optional extra (started early) and holds a check
// action; a decision the extra's condition reads, so the extra is undecided; and an old step
// that is not relevant.
import type { Ready } from "../detail/model.ts";
import type { Level, LevelEdge, LevelNode } from "./model.ts";

const today = "2026-10-06";
const none = { entries: [], total: 0 };
const base = { gravity_from: none, leverage_from: none, actionable: false, gravity: 1, leverage: 0, dates: {} };
const relevant = { relevance: { value: "relevant" as const } };
const owned = { participations: { k_owner: { entities: ["e_one"], origin: "explicit" as const } } };
const bound = (date: string) => ({ date, chain: { constraints: [], fixed: [] } });

export function canvasView(): Ready {
  return {
    status: "ready",
    key: { journey: "j_canvas", revision: 3, deployment_revision: 1, today },
    inputs: {
      today,
      timezone: "UTC",
      deployment: { revision: 1, entities: [{ key: "e_one", name: "Person One" }] },
      rank: { urgency: 0.4, late: 0.15, gravity: 0.25, leverage: 0.2, horizon_days: 14, undecided_discount: 0.5, other_owner_factor: 2 },
    },
    journey: {
      header: { id: "j_canvas", name: "Canvas journey", status: "active", created_at: "2026-10-01T00:00:00Z", created_on: "2026-10-01" },
      revision: 3,
      graph: {
        nodes: [
          { key: "n_kick", id: "kick", kind: "milestone", title: "Kickoff" },
          { key: "n_choose", id: "choose", kind: "decision", title: "Approach", prompt: "Which approach do we take?", answer_type: "boolean" },
          { key: "n_stage", id: "stage", kind: "group", title: "Build stage", opens_at: "n_kick" },
          { key: "n_build", id: "build", parent: "n_stage", kind: "deliverable", title: "Build it", requires: ["n_option"] },
          { key: "n_check", id: "check", parent: "n_build", kind: "action", title: "Check the build" },
          { key: "n_option", id: "option", kind: "deliverable", title: "Optional extra" },
          { key: "n_old", id: "old", kind: "action", title: "Old step" },
        ],
        state: {
          nodes: {
            n_kick: { state: "reached", provenance: "from_route" },
            n_build: { state: "active", provenance: "from_route" },
            n_old: { state: "todo", provenance: "from_route" },
          },
        },
      },
    },
    derived: {
      today,
      frontier: ["n_choose", "n_check"],
      acting_frontier: ["n_choose", "n_check"],
      nodes: {
        n_kick: { ...base, display_state: "done", ...relevant, gravity: 9 },
        n_choose: { ...base, display_state: "ready", ...relevant, ...owned, actionable: true, gravity: 8, leverage: 1, dates: { due: bound("2026-10-09"), latest_start: bound("2026-10-08"), slack_days: 2 } },
        n_stage: { ...base, display_state: "active", ...relevant, gravity: 4 },
        n_build: { ...base, display_state: "active", ...relevant, ...owned, gravity: 4, overdue: true, blocked_by: [{ node: "n_option", via: "explicit" }], dates: { due: bound("2026-10-01") } },
        n_check: { ...base, display_state: "ready", ...relevant, ...owned, actionable: true, gravity: 5, dates: { due: bound("2026-11-30") } },
        n_option: { ...base, display_state: "conditional", relevance: { value: "undecided", decisions: ["n_choose"] }, gravity: 4.5, blocked_by: [{ node: "n_choose", via: { condition: { condition_on: "n_option" } } }] },
        n_old: { ...base, display_state: "not_relevant", relevance: { value: "not_relevant" }, gravity: 0 },
      },
    },
  };
}

/** Each node's display state, as the derive above gives it (D8). */
const SHOWN: Record<string, LevelNode["display_state"]> = {
  n_kick: "done",
  n_choose: "ready",
  n_stage: "active",
  n_build: "active",
  n_check: "ready",
  n_option: "conditional",
  n_old: "not_relevant",
};
const node = (key: string, extra: Partial<LevelNode> = {}): LevelNode => ({ key, display_state: SHOWN[key] ?? "ready", ...extra });
const explicit = (requirement: string, dependent: string): LevelEdge => ({
  from: requirement,
  to: dependent,
  gates: true,
  underlying: [{ requirement, dependent, origin: "explicit", gates: true }],
});
const implicit = (requirement: string, dependent: string, origin: "condition" | "stage_opening"): LevelEdge => ({
  from: requirement,
  to: dependent,
  gates: true,
  implicit: true,
  underlying: [{ requirement, dependent, origin, gates: true }],
});

/** The whole journey with every kind shown. */
export function wholeLevel(): Level {
  return {
    shown: ["group", "decision", "deliverable", "action", "milestone"],
    nodes: [
      node("n_kick"),
      node("n_choose"),
      node("n_stage", { group_state: "active", roll_up: { children_active: true } }),
      node("n_build", { parent: "n_stage", roll_up: { owners: ["e_one"], max_child_gravity: 5, min_child_slack_days: 40 } }),
      node("n_check", { parent: "n_build" }),
      node("n_option"),
      node("n_old"),
    ],
    edges: [implicit("n_choose", "n_option", "condition"), implicit("n_kick", "n_stage", "stage_opening"), explicit("n_option", "n_build")],
  };
}

/** Actions and decisions hidden: the check rolls up into the build, the condition gate goes. */
export function actionsAndDecisionsHidden(): Level {
  return {
    shown: ["group", "deliverable", "milestone"],
    nodes: [
      node("n_kick"),
      node("n_stage", { group_state: "active", roll_up: { children_active: true } }),
      node("n_build", { parent: "n_stage", rolled_up: ["n_check"], roll_up: { owners: ["e_one"] } }),
      node("n_option", { hidden_prerequisites: ["n_choose"] }),
    ],
    edges: [implicit("n_kick", "n_stage", "stage_opening"), explicit("n_option", "n_build")],
  };
}
