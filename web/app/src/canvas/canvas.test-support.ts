// A small derived journey and its canvas levels for the canvas's unit tests, written as the
// derive worker answers them: a kickoff milestone (done) required by a build stage, whose build
// deliverable is active while blocked by an optional extra (started ahead of it) and holds a check
// action; a decision the extra's condition reads, so the extra is undecided; and an old step
// that is not relevant.
import type { Ready } from "../detail/model.ts";
import type { Level, LevelEdge, LevelNode } from "./model.ts";

const today = "2026-10-06";
const none = { entries: [], total: 0 };
const base = { gravity_from: none, unlocks_from: none, waits_on: none, actionable: false, gravity: 1, unlocks: 0, dates: {} };
/** The build stage holds the build and its check, the build is overdue; the build holds the check. */
const stageProgress = { done: 0, total: 2, overdue: 1, decisions_needed: 0 };
const buildProgress = { done: 0, total: 1, overdue: 0, decisions_needed: 0 };
const relevant = { relevance: { value: "relevant" as const }, snoozable: true, open: true };
const owned = { participations: { k_owner: { identities: ["who_one"], origin: "explicit" as const } } };
const bound = (date: string) => ({ date, chain: { constraints: [], fixed: [] } });

const inputs: Ready["inputs"] = {
  today,
  now: `${today}T12:00:00Z`,
  timezone: "UTC",
  deployment: { revision: 1, identities: [{ key: "who_one", name: "Person One" }] },
};

export function canvasView(): Ready {
  return {
    status: "ready",
    key: { journey: "j_canvas", revision: 3, deployment_revision: 1, today, now: `${today}T12:00:00Z` },
    inputs,
    journey: {
      header: { id: "j_canvas", name: "Canvas journey", status: "active", created_at: "2026-10-01T00:00:00Z", created_on: "2026-10-01" },
      revision: 3,
      graph: {
        nodes: [
          { key: "n_kick", id: "kick", kind: "milestone", title: "Kickoff" },
          { key: "n_choose", id: "choose", kind: "decision", title: "Approach", prompt: "Which approach do we take?", answer_type: "boolean" },
          { key: "n_stage", id: "stage", kind: "group", title: "Build stage", requires: ["n_kick"] },
          { key: "n_build", id: "build", parent: "n_stage", kind: "deliverable", title: "Build it", requires: ["n_option"] },
          { key: "n_check", id: "check", parent: "n_build", kind: "action", title: "Check the build" },
          { key: "n_option", id: "option", kind: "deliverable", title: "Optional extra", relevant_when: { answered: "n_choose" } },
          { key: "n_scope", id: "scope", kind: "decision", title: "Scope", prompt: "Is the old step in scope?", answer_type: "boolean" },
          { key: "n_old", id: "old", kind: "action", title: "Old step", relevant_when: { not: { answered: "n_scope" } } },
        ],
        state: {
          nodes: {
            n_kick: { state: "done" },
            n_build: { state: "active" },
            n_old: { state: "todo" },
            n_scope: { state: "decided" },
          },
          answers: { n_scope: { boolean: true } },
        },
      },
    },
    derived: {
      today, now: inputs.now,
      frontier: ["n_choose"],
      acting_frontier: ["n_choose"],
      current_stages: ["n_stage"],
      nodes: {
        n_kick: { ...base, display_state: "done", ...relevant, snoozable: false, open: false, moves: ["reopen"], gravity: 9 },
        n_choose: { ...base, display_state: "ready", ...relevant, ...owned, actionable: true, moves: ["answer", "skip"], gravity: 8, unlocks: 1, dates: { due: bound("2026-10-09"), latest_start: bound("2026-10-08"), slack_days: 2 } },
        n_stage: { ...base, display_state: "active", ...relevant, skip_count: 2, moves: ["insert_route", "mark_done", "skip"], gravity: 4, blocked_by: [{ node: "n_build", via: "containment" }], progress: stageProgress },
        n_build: {
          ...base,
          display_state: "active",
          ...relevant,
          ...owned,
          gravity: 4,
          overdue: true,
          skip_count: 1,
          moves: ["complete", "insert_route", "skip", "stop"],
          blocked_by: [{ node: "n_option", via: "explicit" }, { node: "n_check", via: "containment" }],
          waits_on: { entries: [{ node: "n_option", via: "explicit" }], total: 1 },
          progress: buildProgress,
          dates: { due: bound("2026-10-01"), overdue_days: 5 },
        },
        n_check: { ...base, display_state: "blocked", ...relevant, ...owned, moves: ["complete", "insert_route", "skip", "start"], gravity: 5, dates: { due: bound("2026-11-30") } },
        n_option: {
          ...base,
          display_state: "conditional",
          relevance: { value: "undecided", decisions: ["n_choose"], unanswered: ["n_choose"] },
          snoozable: true,
          open: true,
          moves: ["complete", "insert_route", "skip", "start"],
          gravity: 4.5,
          blocked_by: [{ node: "n_choose", via: { condition: { condition_on: "n_option" } } }],
          waits_on: { entries: [{ node: "n_choose", via: { condition: { condition_on: "n_option" } } }], total: 1 },
        },
        n_old: { ...base, display_state: "not_relevant", relevance: { value: "not_relevant" }, moves: ["insert_route", "skip"], gravity: 0 },
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
  underlying: [{ requirement, dependent, origin: "explicit" }],
});
const implicit = (requirement: string, dependent: string, origin: "condition"): LevelEdge => ({
  from: requirement,
  to: dependent,
  implicit: true,
  underlying: [{ requirement, dependent, origin }],
});

/** The whole journey with every kind shown. */
export function wholeLevel(): Level {
  return {
    shown: ["group", "decision", "deliverable", "action", "milestone"],
    nodes: [
      node("n_kick"),
      node("n_choose"),
      node("n_stage", { roll_up: { children_active: true, progress: stageProgress } }),
      node("n_build", { parent: "n_stage", roll_up: { owners: ["who_one"], subtree_gravity: 9, min_child_slack_days: 40, progress: buildProgress } }),
      node("n_check", { parent: "n_build" }),
      node("n_option"),
      node("n_old"),
    ],
    edges: [implicit("n_choose", "n_option", "condition"), explicit("n_kick", "n_stage"), explicit("n_option", "n_build")],
    date_links: [],
  };
}

/** Actions and decisions hidden: the check rolls up into the build, the condition gate goes. */
export function actionsAndDecisionsHidden(): Level {
  return {
    shown: ["group", "deliverable", "milestone"],
    nodes: [
      node("n_kick"),
      node("n_stage", { roll_up: { children_active: true, progress: stageProgress } }),
      node("n_build", { parent: "n_stage", rolled_up: ["n_check"], roll_up: { owners: ["who_one"], progress: buildProgress } }),
      node("n_option", { hidden_prerequisites: ["n_choose"] }),
    ],
    edges: [explicit("n_kick", "n_stage"), explicit("n_option", "n_build")],
    date_links: [],
  };
}
