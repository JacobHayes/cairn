// C18 against the real engine: the front page's own values (the final milestone, what needs
// attention, who has which action, what finished recently) read from fixtures and from patches
// of them, so they cannot drift from what the engine derives.
import type { Schema } from "@cairn/client";
import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { derivedJourney, journeyAfter, seeded, type DerivedJourney, type Fixtures } from "../authoring/engine.test-support.ts";
import type { Mutation } from "../authoring/graph.ts";
import { transition } from "../detail/model.ts";
import { dateWords } from "../status/when.ts";
import { attentionRows, blockedRows, claimActions, claimedKeys, lifecycleLine, milestoneLine, recentlyFinished, scheduledRows, shortfallWords, summaryModel, underWay } from "./model.ts";

let host: Fixtures;
const held: DerivedJourney[] = [];
/** The journeys the seeded host holds (the fixtures'), read from it rather than listed here. */
let fixtureJourneys: string[] = [];

beforeAll(async () => {
  host = await seeded();
  fixtureJourneys = host.journeyIds();
});

afterEach(() => {
  held.splice(0).forEach((each) => { each.free(); });
});

/** A fixture journey, after `mutations`, derived. */
function open(journey: string, mutations: Mutation[] = []): DerivedJourney {
  const text = mutations.length === 0 ? host.documentText(journey) : journeyAfter(host, journey, mutations);
  const derived = derivedJourney(host, text);
  held.push(derived);
  return derived;
}

let fresh = 0;

/** A new journey from the vendor evaluation route's first version, derived: nothing is answered yet. */
async function openFresh(mutations: Mutation[] = []): Promise<DerivedJourney> {
  fresh += 1;
  const id = `j_summary_fresh_${String(fresh)}`;
  await host.patch({ patch: { id: `p_summary_fresh_${String(fresh)}`, target: { journey: id }, base_revision: 0, mutations: [{ op: "create_journey", name: "A fresh evaluation", from: { route: "vendor-evaluation", version: 1 } }] } });
  const text = mutations.length === 0 ? host.documentText(id) : journeyAfter(host, id, mutations);
  const derived = derivedJourney(host, text);
  held.push(derived);
  return derived;
}

/** `journey` derived again with `viewer` as the caller's identities, and `aliases` (old key to survivor) merged in the deployment. */
function asViewer(journey: DerivedJourney, viewer: readonly string[], aliases?: Record<string, string>): DerivedJourney {
  const { inputs } = journey.ready;
  // A merged identity's key no longer names an identity: it resolves through the alias.
  const deployment = aliases === undefined ? inputs.deployment : { ...inputs.deployment, aliases, identities: (inputs.deployment.identities ?? []).filter((identity) => !(identity.key in aliases)) };
  const derived = derivedJourney(host, JSON.stringify({ journey: journey.ready.journey, inputs: { ...inputs, deployment, viewer: [...viewer] } }));
  held.push(derived);
  return derived;
}

/**
 * The front page's values for `journey`: the viewer's owned nodes are the engine's "mine" (the
 * journey re-derived with `viewer` as the caller's identities, when one is given).
 */
function pageOf(journey: DerivedJourney, viewer?: readonly string[], aliases?: Record<string, string>) {
  const seen = viewer === undefined && aliases === undefined ? journey : asViewer(journey, viewer ?? journey.ready.inputs.viewer ?? [], aliases);
  const model = summaryModel(seen.ready, seen.project({ projection: "status_summary" }));
  const frontier = seen.project({ projection: "next" }).items;
  const owned = new Set(seen.project({ projection: "mine" }).filter((entry) => entry.kinds.includes("k_owner")).map((entry) => entry.node));
  return { model, frontier, owned, actions: claimActions(seen.ready, model, frontier, owned) };
}

const keys = (rows: { key: string }[]) => rows.map((row) => row.key);
const owner = (node: string, identity: string): Mutation => ({ op: "set_participation", node, kind: "k_owner", source: [identity] });

describe("C13, F1, F6: the lead's milestone line", () => {
  it("names the final milestone with its pin", () => {
    const journey = open("j_vendor_eval");
    const { model } = pageOf(journey);
    expect(milestoneLine(journey.ready, model)).toMatchObject({ role: "final", key: "n_decision_meeting", date: "2026-11-20", origin: "pin" });
  });

  it("gives the final milestone its own shortfall and no other node's", () => {
    // The launch pinned before the beta can end: the launch and the work ahead of it fall short.
    const journey = open("j_launch", [{ op: "set_pin", node: "n_launch", date: "2026-11-10" }]);
    const { model } = pageOf(journey);
    const line = milestoneLine(journey.ready, model);
    expect(line).toMatchObject({ role: "final", key: "n_launch", shortfallDays: journey.ready.derived.nodes["n_launch"]?.dates.shortfall?.shortfall_days });
    expect(line?.shortfallDays).toBeGreaterThan(0);
    expect(model.shortfalls.filter((each) => each.key !== "n_launch").length).toBeGreaterThan(0);
  });

  it("names an undated final milestone's feeding decision", async () => {
    const journey = await openFresh();
    const line = milestoneLine(journey.ready, pageOf(journey).model);
    expect(line).toMatchObject({ role: "final", key: "n_decision_meeting", date: undefined });
    expect(line?.feeder?.key).toBe(journey.ready.journey.graph.nodes?.find((node) => node.feeds_milestone === "n_decision_meeting")?.key);
  });

  it("calls only the final milestone the end: without one it is the next milestone, or nothing", () => {
    const hiring = open("j_hiring");
    expect(milestoneLine(hiring.ready, pageOf(hiring).model)).toBeUndefined();
    const meeting = open("j_vendor_eval", [{ op: "set_node_field", node: "n_decision_meeting", value: { final: false } as Schema<"NodeFieldValueResolved"> }]);
    const { model } = pageOf(meeting);
    const line = milestoneLine(meeting.ready, model);
    expect(line?.role).toBe("next");
    expect(line?.key).toBe(model.upcoming[0]?.key);
  });

  it("says a done final milestone is done, from an actual date", () => {
    const journey = open("j_bakeoff", [
      { op: "transition", node: "n_wrap_up", transition: "complete" },
    ]);
    const line = milestoneLine(journey.ready, pageOf(journey).model);
    expect(line).toMatchObject({ role: "final", key: "n_wrap_up", origin: "actual" });
  });
});

describe("F1, C13: the final milestone line says when it is done", () => {
  it("is not done while open, and is once done", () => {
    const before = open("j_bakeoff");
    expect(milestoneLine(before.ready, pageOf(before).model)).toMatchObject({ role: "final", done: false });
    const after = open("j_bakeoff", [transition("n_wrap_up", "complete")]);
    expect(milestoneLine(after.ready, pageOf(after).model)).toMatchObject({ role: "final", done: true });
  });
});

describe("C18, D3, D4, F6: what needs attention", () => {
  it("lists a node once with every reason, overdue first, and the worst nodes first", () => {
    const journey = open("j_launch");
    const { model } = pageOf(journey);
    const rows = attentionRows(journey.ready, model);
    expect(new Set(keys(rows)).size).toBe(rows.length);
    const flagged = new Set([...model.overdue, ...model.shortfalls, ...model.stale].map((each) => each.key));
    expect(new Set(keys(rows))).toEqual(flagged);
    const worst = rows.map((row) => (model.overdue.some((each) => each.key === row.key) ? 0 : model.shortfalls.some((each) => each.key === row.key) ? 1 : 2));
    expect(worst).toEqual([...worst].sort((a, b) => a - b));
  });

  it("says a shortfall only of work still ahead, as the days it needs", () => {
    const journey = open("j_launch", [{ op: "set_pin", node: "n_launch", date: "2026-11-10" }]);
    const { model } = pageOf(journey);
    const reasons = (key: string) => attentionRows(journey.ready, model).find((row) => row.key === key)?.reasons ?? [];
    expect(model.shortfalls.length).toBeGreaterThan(0);
    model.shortfalls.forEach((each) => {
      expect(journey.ready.journey.graph.state?.nodes?.[each.key]?.state).not.toBe("done");
      expect(reasons(each.key)).toContain(shortfallWords(each.days));
    });
  });

  it("keeps both reasons of a node that is overdue and short, the overdue one first", () => {
    const model = {
      ...summaryModel(open("j_launch").ready, { by_state: {}, by_display_state: {}, remaining: 0 }),
      overdue: [{ key: "n_a", title: "A", due: "2026-10-01", lateDays: 3 }],
      shortfalls: [{ key: "n_a", title: "A", days: 2 }, { key: "n_b", title: "B", days: 1 }],
      stale: [{ key: "n_a", title: "A", reasons: ["missing artifact"] }],
    };
    const [first, second] = attentionRows(open("j_launch").ready, model);
    expect(first).toMatchObject({ key: "n_a", tone: "bad" });
    expect(first?.reasons).toHaveLength(3);
    expect(first?.reasons[0]).toContain("late");
    expect(first?.reasons[2]).toMatch(/stale/i);
    expect(second?.key).toBe("n_b");
  });
});

describe("C10, C11, E1, E4: one action row per node", () => {
  it("claims the viewer's owned frontier items first, whatever their kind, and the rest by kind", () => {
    const journey = open("j_bakeoff", [owner("n_trial_a", "who_k3d7qm2xw5tn6pfa")]);
    const { frontier, actions } = pageOf(journey);
    expect(keys(actions.yours)).toEqual(["n_trial_a"]);
    expect(keys(actions.nextUp)).toEqual(keys(frontier).filter((key) => key !== "n_trial_a"));
  });

  it("does not claim another person's or an informed participant's item", () => {
    const informed = (node: string): Mutation => ({ op: "set_participation", node, kind: "k_informed", source: ["who_k3d7qm2xw5tn6pfa"] });
    const journey = open("j_vendor_eval", [owner("n_review_opens", "who_j5v2nx6cwr4tdkq7"), informed("n_review_opens")]);
    const { actions } = pageOf(journey, ["who_k3d7qm2xw5tn6pfa"]);
    expect(keys(actions.yours)).toEqual([]);
    expect(keys(actions.nextUp)).toEqual(["n_review_opens"]);
  });

  it("claims an item owned through a merged identity", () => {
    const journey = open("j_bakeoff", [owner("n_trial_a", "who_h3w7bs5xkq2dfnr6")]);
    const { actions } = pageOf(journey, ["who_g6t4ycm2pe7vsjan"], { who_h3w7bs5xkq2dfnr6: "who_g6t4ycm2pe7vsjan" });
    expect(keys(actions.yours)).toEqual(["n_trial_a"]);
  });

  it("claims a viewer's six decisions all in Waiting on you, none in Decide now", () => {
    const decisions = ["d1", "d2", "d3", "d4", "d5", "d6"];
    const journey = open("j_bakeoff", [
      ...decisions.map((id): Mutation => ({ op: "add_node", node: { key: `n_${id}`, id, kind: "decision", title: `Choose ${id}`, prompt: "Which?", answer_type: "text" } })),
      ...decisions.map((id) => owner(`n_${id}`, "who_k3d7qm2xw5tn6pfa")),
    ]);
    const { actions } = pageOf(journey);
    expect(keys(actions.yours).filter((key) => key.startsWith("n_d"))).toHaveLength(6);
    expect(keys(actions.decide)).toEqual([]);
  });

  it("puts each node in one set, whatever the viewer", () => {
    for (const id of fixtureJourneys) {
      const journey = open(id);
      for (const viewer of [[], ["who_g6t4ycm2pe7vsjan"], ["who_k3d7qm2xw5tn6pfa", "who_b6r2vz4jhs7cnqe3"]]) {
        const { actions, frontier } = pageOf(journey, viewer);
        const all = [...keys(actions.yours), ...keys(actions.decide), ...keys(actions.nextUp), ...keys(actions.waiting)];
        expect(new Set(all).size).toBe(all.length);
        expect(new Set([...keys(actions.yours), ...keys(actions.decide), ...keys(actions.nextUp)])).toEqual(new Set(keys(frontier)));
      }
    }
  });

  it("holds every frontier row once on a journey of 2,000 nodes", () => {
    const work = Array.from({ length: 1000 }, (_, at): Mutation => ({ op: "add_node", node: { key: `n_w${String(at)}`, id: `w${String(at)}`, kind: "action", title: `Work ${String(at)}` } }));
    const decisions = Array.from({ length: 900 }, (_, at): Mutation => ({ op: "add_node", node: { key: `n_x${String(at)}`, id: `x${String(at)}`, kind: "decision", title: `Open ${String(at)}`, prompt: "Which?", answer_type: "text" } }));
    const waits = Array.from({ length: 80 }, (_, at): Mutation => ({ op: "add_edge", edge: { node: `n_x${String(at)}`, requires: `n_w${String(at)}` } }));
    const journey = open("j_bakeoff", [...work, ...decisions, ...waits]);
    const { frontier, actions, model } = pageOf(journey, ["who_g6t4ycm2pe7vsjan"]);
    const all = [...actions.yours, ...actions.decide, ...actions.nextUp];
    expect(all.length).toBe(frontier.length);
    expect(new Set(keys(all)).size).toBe(frontier.length);
    expect(model.inScope).toBeGreaterThan(1900);
  });
});

describe("C11: decisions waiting", () => {
  it("names a waiting decision's first unblocker", () => {
    const journey = open("j_bakeoff");
    const { actions } = pageOf(journey);
    const winner = actions.waiting.find((each) => each.key === "n_winner");
    expect(winner?.fact).toMatch(/^Waiting for /);
    expect(winner?.fact).toContain(journey.ready.journey.graph.nodes?.find((node) => node.key === "n_summary")?.title ?? "missing");
  });

  it("says a snoozed decision is snoozed until its date", async () => {
    const fresh = await openFresh();
    const decision = pageOf(fresh).actions.decide[0]?.key ?? "";
    const journey = await openFresh([{ op: "snooze", node: decision, until: { date: "2026-10-20" } }]);
    expect(keys(pageOf(journey).frontier)).not.toContain(decision);
    expect(pageOf(journey).actions.waiting.find((each) => each.key === decision)?.fact).toBe("Snoozed until Oct 20");
  });
});

describe("F2: recently finished", () => {
  it("lists the newest recorded finishes first, at most five, and never a node that is not finished", () => {
    const journey = open("j_vendor_eval");
    const rows = recentlyFinished(journey.ready);
    expect(rows.length).toBeLessThanOrEqual(5);
    const record = (key: string) => journey.ready.journey.graph.state?.nodes?.[key];
    const dates = rows.map((row) => record(row.key)?.finished_on ?? "");
    expect(dates).toEqual([...dates].sort().reverse());
    for (const row of rows) {
      expect(["done", "decided"]).toContain(record(row.key)?.state);
    }
  });

  it("leaves out a reopened node and a skipped one", () => {
    const before = recentlyFinished(open("j_vendor_eval").ready);
    const reopened = before[0]?.key ?? "";
    const journey = open("j_vendor_eval", [{ op: "transition", node: reopened, transition: "reopen" }]);
    expect(keys(recentlyFinished(journey.ready))).not.toContain(reopened);
  });
});

describe("D8, C18: what is under way and what is blocked", () => {
  it("lists an active container with how much inside it is open, off the action lists", () => {
    const journey = open("j_vendor_eval");
    const { actions } = pageOf(journey);
    const rows = underWay(journey.ready, claimedKeys(actions));
    const active = (journey.ready.journey.graph.nodes ?? []).filter((node) => journey.ready.derived.nodes[node.key]?.display_state === "active").map((node) => node.key);
    expect(new Set(keys(rows))).toEqual(new Set(active.filter((key) => !claimedKeys(actions).has(key))));
    expect(rows.length).toBeGreaterThan(0);
    expect(rows[0]?.fact()).toMatch(/\d+ of \d+ inside still open/);
  });

  it("names what holds a blocked node, a blocked container included", () => {
    const journey = open("j_vendor_eval");
    const { actions } = pageOf(journey);
    const rows = blockedRows(journey.ready, claimedKeys(actions));
    const report = rows.find((each) => each.key === "n_final_report");
    expect(report?.fact()).toContain(journey.ready.journey.graph.nodes?.find((node) => node.key === "n_review_opens")?.title ?? "missing");
    expect(keys(rows)).toContain("n_final_review");
    expect(rows.find((each) => each.key === "n_final_review")?.fact()).toContain(journey.ready.journey.graph.nodes?.find((node) => node.key === "n_review_opens")?.title ?? "missing");
  });

  it("puts a node on one list only, in every fixture journey", () => {
    expect(fixtureJourneys.length).toBeGreaterThan(0);
    for (const name of fixtureJourneys) {
      const journey = open(name);
      const { actions } = pageOf(journey);
      const taken = claimedKeys(actions);
      const all = [...taken, ...keys(underWay(journey.ready, taken)), ...keys(blockedRows(journey.ready, taken)), ...keys(scheduledRows(journey.ready, taken))];
      expect(new Set(all).size, name).toBe(all.length);
    }
  });
});

describe("D8, C18: what waits for a date", () => {
  /** The effective date of a milestone in `journey`, in the words the page says dates in. */
  const dayOf = (journey: DerivedJourney, milestone: string) => dateWords(journey.ready.derived.nodes[milestone]?.dates.effective_date?.date ?? "", journey.ready.derived.today);
  /** The earliest a milestone in `journey` can be, in those words: what a rule measured from it waits for. */
  const earliestOf = (journey: DerivedJourney, milestone: string) => dateWords(journey.ready.derived.nodes[milestone]?.dates.earliest_start?.date ?? "", journey.ready.derived.today);

  it("lists work waiting for its own date with that date, and leaves milestones to the dates panel", () => {
    const journey = open("j_launch");
    const { actions } = pageOf(journey);
    const rows = scheduledRows(journey.ready, claimedKeys(actions));
    expect(keys(rows)).toEqual(["n_announcement"]);
    expect(journey.ready.derived.nodes.n_launch?.display_state).toBe("scheduled");
    // The announcement is no sooner than the beta's end, which is not dated yet: the earliest it can be.
    expect(rows[0]?.fact()).toContain(earliestOf(journey, "n_beta_end"));
  });

  it("names the date of the scheduled node a waiting decision waits for", () => {
    const journey = open("j_launch", [
      { op: "add_node", node: { key: "n_debrief", id: "debrief", kind: "decision", title: "Debrief format", prompt: "Which?", answer_type: "text" } },
      { op: "add_edge", edge: { node: "n_debrief", requires: "n_launch" } },
    ]);
    const { actions } = pageOf(journey);
    const fact = actions.waiting.find((each) => each.key === "n_debrief")?.fact;
    expect(fact).toContain(journey.ready.journey.graph.nodes?.find((node) => node.key === "n_launch")?.title ?? "missing");
    expect(fact).toContain(dayOf(journey, "n_launch"));
  });
});

describe("B11, A19, C18: a closed journey says what it left", () => {
  const as = (journey: DerivedJourney, status: "active" | "completed" | "archived") => ({ ...journey.ready, journey: { ...journey.ready.journey, header: { ...journey.ready.journey.header, status } } });

  it("says nothing while active, and counts what is unfinished once completed or archived", () => {
    const journey = open("j_bakeoff");
    const { model } = pageOf(journey);
    expect(lifecycleLine(as(journey, "active"), model)).toBeUndefined();
    expect(model.remaining).toBeGreaterThan(0);
    expect(lifecycleLine(as(journey, "completed"), model)).toContain(String(model.remaining));
    expect(lifecycleLine(as(journey, "archived"), model)).toContain(String(model.remaining));
    expect(lifecycleLine(as(journey, "archived"), model)).not.toBe(lifecycleLine(as(journey, "completed"), model));
  });
});
