// A small derived journey for the acting surfaces' unit tests, built by patching a new journey
// in the real engine and derived by it: a meeting milestone and an unassigned decision on the
// acting frontier; a deliverable that requires an artifact; a placeholder still to break down;
// a decision snoozed until the deliverable is done; a later decision that requires the meeting;
// a stage the meeting opens, holding a decision; a finished action; and a decision no longer
// in scope. The derived values are the engine's, so no state here is one it cannot produce.
import type { Mutation } from "../authoring/graph.ts";
import { builtJourney, loadedEngine } from "../authoring/engine.test-support.ts";
import type { Ready } from "../detail/model.ts";

await loadedEngine();

const decision = (key: string, title: string, extra: Record<string, unknown> = {}): Mutation =>
  ({ op: "add_node", node: { key, id: key.slice(2), kind: "decision", title, prompt: title, answer_type: "boolean", ...extra } });
const work = (key: string, kind: string, title: string, extra: Record<string, unknown> = {}): Mutation =>
  ({ op: "add_node", node: { key, id: key.slice(2), kind, title, ...extra } }) as Mutation;

/** A date rule: this many days after the journey was created (the engine's clock reads 2026-10-06). */
const startingAfter = (days: number) => ({ after: "journey.created_at", offset: days });

const mutations: Mutation[] = [
  work("n_meet", "milestone", "Meeting"),
  decision("n_pick", "Pick one"),
  work("n_work", "deliverable", "Write it", { requires_artifact: true }),
  work("n_hold", "action", "Break me down", { placeholder: true }),
  work("n_log", "action", "Write the log", { requires_note: true }),
  decision("n_wait", "Wait for it", { not_before: startingAfter(0) }),
  decision("n_later", "Decide later", { requires: ["n_meet"], not_before: startingAfter(14) }),
  work("n_stage", "group", "Stage", { opens_at: "n_meet" }),
  decision("n_inner", "Inside the stage", { parent: "n_stage" }),
  work("n_done", "action", "Finished"),
  decision("n_gate", "Is it needed?"),
  decision("n_gone", "Out of scope", { relevant_when: { not: { answered: "n_gate" } } }),
  { op: "snooze", node: "n_wait", until: { node: "n_work" } },
  { op: "transition", node: "n_done", transition: "complete" },
  { op: "answer", decision: "n_gate", value: { boolean: false } },
] as Mutation[];

/** The journey above, derived by the engine. */
export function actingView(): Ready {
  return builtJourney("j_act", "Acting journey", mutations);
}
