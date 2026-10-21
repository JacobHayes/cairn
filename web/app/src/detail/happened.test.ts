// J1, C8: what each event type says happened. Each type is checked for the facts its sentence
// is made from (who, which node, which state, which date), and every sentence for naming
// nothing by key. The words themselves are not pinned.
import type { Schema } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { happened, noPrior, recount, remember, type Facts, type Words } from "./happened.ts";

type Event = Schema<"Event">;
type EventType = Schema<"EventType">;
type Write = Schema<"Write">;
type GraphRecord = Schema<"GraphRecord">;

const TITLES: Record<string, string> = { n_plan: "Plan the launch", n_review: "Review the plan", n_decide: "Pick a venue" };
const words: Words = {
  viewed: "n_plan",
  node: (key) => TITLES[key] ?? "an item that was removed",
  identity: (key) => (key === "who_ana" ? "Ana Reyes" : "Someone"),
  role: (key) => (key === "r_lead" ? "Lead" : "A role"),
  kind: (key) => (key === "k_owner" ? "owner" : "Reviewer"),
  day: (date) => `on ${date}`,
  person: (user) => (user === "u_boss" ? "Bo Boss" : "Someone"),
};

const put = (record: GraphRecord): Write => ({ put: { graph: { graph: { journey: "j_test" }, record } } });
const state = (node: string, fields: Partial<Schema<"NodeState">> & { state: Schema<"State"> }): Write =>
  put({ node_state: { node, state: { provenance: "local", ...fields } } });
const field = (node: string, value: Schema<"NodeFieldValueResolved">): Write => put({ node_field: { node, value } });

function event(type: EventType, subject: Event["subject"], delta: Write[] = [], more: Partial<Event> = {}): Event {
  return { patch_id: "p_one", ordinal: 0, log: { journey: "j_test" }, event_type: type, actor: { user: "u_ana" }, subject, at: "2026-10-06T12:00:00Z", delta, ...more };
}

const plan = { node: "n_plan" };

interface Case {
  event: Event;
  /** The events before it that the sentence depends on. */
  after?: Event[];
  facts?: Facts;
}

const CASES: Record<EventType, Case> = {
  journey_created: { event: event("journey_created", { journey: "j_test" }) },
  journey_edited: { event: event("journey_edited", { journey: "j_test" }) },
  journey_status_changed: { event: event("journey_status_changed", { journey: "j_test" }) },
  journey_deleted: { event: event("journey_deleted", { journey: "j_test" }) },
  route_created: { event: event("route_created", { route: "r_launch" }) },
  route_edited: { event: event("route_edited", { route: "r_launch" }) },
  route_retired: { event: event("route_retired", { route: "r_launch" }) },
  route_unretired: { event: event("route_unretired", { route: "r_launch" }) },
  draft_opened: { event: event("draft_opened", { route: "r_launch" }) },
  route_version_imported: { event: event("route_version_imported", { route: "r_launch" }) },
  saved_as_route: { event: event("saved_as_route", { route: "r_launch" }) },
  draft_discarded: { event: event("draft_discarded", { route: "r_launch" }) },
  route_published: { event: event("route_published", { route: "r_launch" }) },
  node_added: { event: event("node_added", plan, [put({ node: { key: "n_plan", id: "plan", kind: "deliverable", title: "Plan the launch" } })]), facts: { title: "Plan the launch" } },
  node_changed: {
    event: event("node_changed", plan, [field("n_plan", { title: "Plan the event" })]),
    after: [event("node_added", plan, [put({ node: { key: "n_plan", id: "plan", kind: "deliverable", title: "Plan the launch" } })])],
    facts: { title: "Plan the event", renamedFrom: "Plan the launch", fields: ["title"] },
  },
  weight_changed: { event: event("weight_changed", plan, [field("n_plan", { weight: 3 })]), facts: { fields: ["weight"] } },
  node_removed: { event: event("node_removed", plan, [{ remove: { graph: { journey: "j_test" } } }]) },
  edge_changed: { event: event("edge_changed", { edge: { node: "n_plan", requires: "n_review" } }, [put({ edge: { node: "n_plan", requires: "n_review" } })]) },
  role_changed: { event: event("role_changed", { role: "r_lead" }) },
  participation_kind_changed: { event: event("participation_kind_changed", { kind: "k_owner" }) },
  default_owner_changed: { event: event("default_owner_changed", plan, [put({ default_owner: "r_lead" })]) },
  participation_changed: { event: event("participation_changed", plan, [put({ participation: { node: "n_plan", kind: "k_owner", source: ["who_ana"] } })]), facts: { identities: ["Ana Reyes"] } },
  resource_changed: { event: event("resource_changed", plan, [put({ resource: { node: "n_plan", resource: { key: "a_tip", tip: "Start early", title: "A tip" } } })]) },
  segment_inserted: { event: event("segment_inserted", { insertion: "i_one" }) },
  node_transitioned: { event: event("node_transitioned", plan, [state("n_plan", { state: "active" })]), facts: { state: "active" } },
  answer_set: { event: event("answer_set", { node: "n_decide" }, [put({ answer: { decision: "n_decide", value: { boolean: true } } })]) },
  recorded_date_changed: { event: event("recorded_date_changed", plan, [state("n_plan", { state: "done", finished_on: "2026-10-05" })]), facts: { date: "2026-10-05", recorded: "finished" } },
  role_fill_changed: { event: event("role_fill_changed", { role: "r_lead" }, [put({ role_fill: { role: "r_lead", identities: ["who_ana"] } })]), facts: { identities: ["Ana Reyes"] } },
  date_pinned: { event: event("date_pinned", plan, [put({ pin: { node: "n_plan", date: "2026-10-31" } })]), facts: { date: "2026-10-31" } },
  date_shifted: { event: event("date_shifted", plan, [put({ pin: { node: "n_plan", date: "2026-11-02" } })]), facts: { date: "2026-11-02" } },
  date_unpinned: { event: event("date_unpinned", plan) },
  snooze_set: { event: event("snooze_set", plan, [put({ snooze: { node: "n_plan", until: { date: "2026-10-31" } } })]), facts: { date: "2026-10-31" } },
  unsnoozed: { event: event("unsnoozed", plan) },
  override_applied: { event: event("override_applied", plan, [put({ overrides: { node: "n_plan", overrides: { keep: "Still needed" } } })]), facts: { reason: "Still needed" } },
  guard_bypassed: { event: event("guard_bypassed", plan) },
  override_removed: { event: event("override_removed", plan, [put({ overrides: { node: "n_plan", overrides: {} } })]) },
  atomic_changed: { event: event("atomic_changed", plan, [state("n_plan", { state: "todo", atomic: true })]) },
  annotation_added: { event: event("annotation_added", { attachment: "a_note" }) },
  annotation_edited: { event: event("annotation_edited", { attachment: "a_note" }) },
  annotation_removed: { event: event("annotation_removed", { attachment: "a_note" }) },
  journey_upgraded: { event: event("journey_upgraded", { journey: "j_test" }) },
  relinked: { event: event("relinked", { journey: "j_test" }) },
  provenance_changed: { event: event("provenance_changed", plan, [state("n_plan", { state: "todo", provenance: "from_route" })]) },
  local_edit_changed: { event: event("local_edit_changed", plan) },
  identity_created: { event: event("identity_created", { identity: "who_ana" }, [{ put: { identity: { key: "who_ana", name: "Ana Reyes" } } }]) },
  identity_edited: { event: event("identity_edited", { identity: "who_ana" }, [{ put: { identity: { key: "who_ana", name: "Ana Reyes" } } }]) },
  identities_merged: { event: event("identities_merged", { identity: "who_ana" }, [{ put: { identity: { key: "who_ana", name: "Ana Reyes" } } }]) },
  identity_deleted: { event: event("identity_deleted", { identity: "who_ana" }, [{ remove: { identity: "who_ana" } }]) },
  proposal_created: { event: event("proposal_created", { proposal: "pr_one" }) },
  proposal_edited: { event: event("proposal_edited", { proposal: "pr_one" }) },
  proposal_applied: { event: event("proposal_applied", { proposal: "pr_one" }, [], { confirming_user: "u_boss" }), facts: { confirmedBy: "u_boss" } },
  proposal_discarded: { event: event("proposal_discarded", { proposal: "pr_one" }) },
};

/** A sentence that names something by its key: an id like `n_plan`, `u_ana`, `who_ana`, `pr_one`. */
const KEY = /\b(?:[a-z]{1,2})_[a-z0-9-]+\b/;

function told(each: Case) {
  const prior = noPrior();
  each.after?.forEach((earlier) => {
    remember(prior, earlier);
  });
  return happened(each.event, words, prior);
}

describe("happened", () => {
  it.each(Object.entries(CASES))("%s: who did it, and the facts it is made from", (type, each) => {
    const said = told(each);
    expect(said.type).toBe(type);
    expect(said.actor).toBe("u_ana");
    // A case that names no facts asserts there are none, so a fact that appears unannounced is a failure.
    expect(said.facts).toEqual(each.facts === undefined ? {} : expect.objectContaining(each.facts));
    expect(said.what).not.toMatch(KEY);
    expect(said.what).not.toBe("");
  });

  it("tells a stop from a reopen by the state the node was in", () => {
    const todo = event("node_transitioned", plan, [state("n_plan", { state: "todo" })]);
    const started = event("node_transitioned", plan, [state("n_plan", { state: "active" })]);
    const done = event("node_transitioned", plan, [state("n_plan", { state: "done" })]);
    const [, stopped, , reopened] = recount([started, todo, done, todo], words);
    expect([stopped?.facts.from, stopped?.facts.state]).toEqual(["active", "todo"]);
    expect([reopened?.facts.from, reopened?.facts.state]).toEqual(["done", "todo"]);
    expect(stopped?.what).not.toBe(reopened?.what);
  });

  it("tells the recorded date that was edited from the one left alone", () => {
    const finish = event("recorded_date_changed", plan, [state("n_plan", { state: "done", started_on: "2026-10-01", finished_on: "2026-10-05" })]);
    const restart = event("recorded_date_changed", plan, [state("n_plan", { state: "done", started_on: "2026-10-02", finished_on: "2026-10-05" })]);
    const [, edited] = recount([finish, restart], words);
    expect([edited?.facts.recorded, edited?.facts.date]).toEqual(["started", "2026-10-02"]);
  });

  it("says a snooze on a node it waits for by that node's title", () => {
    const snooze = event("snooze_set", plan, [put({ snooze: { node: "n_plan", until: { node: "n_review" } } })]);
    expect(happened(snooze, words).facts.waitsFor).toBe("n_review");
    expect(happened(snooze, words).what).toContain(TITLES.n_review);
  });

  it("names a node that is not the viewed one by its title, a removed one without its key", () => {
    const other = event("node_transitioned", { node: "n_review" }, [state("n_review", { state: "done" })]);
    const gone = event("node_transitioned", { node: "n_gone" }, [state("n_gone", { state: "done" })]);
    expect(happened(other, words).what).toContain("Review the plan");
    expect(happened(gone, words).what).not.toMatch(KEY);
  });

  it("does not name a renamed node by its new title when the old one is not known", () => {
    const renamed = event("node_changed", { node: "n_review" }, [field("n_review", { title: "Review the plan" })]);
    expect(happened(renamed, words).what.match(/Review the plan/g)).toHaveLength(1);
  });

  it("notes when an agent acted for the user", () => {
    expect(happened(event("unsnoozed", plan, [], { actor: { user: "u_ana", agent: "a_one" } }), words).byAgent).toBe(true);
    expect(happened(event("unsnoozed", plan), words).byAgent).toBe(false);
  });
});
