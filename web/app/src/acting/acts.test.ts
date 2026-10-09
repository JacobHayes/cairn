// C11's per-kind actions (a placeholder offers no done until it is broken down or atomic, B10),
// C11 done with its inline artifact (G2), D2's inline assign, and C9's bulk actions: one patch,
// one mutation per node, nothing sent when a selected node cannot take the action.
import { describe, expect, it } from "vitest";

import type { Mutation } from "../detail/model.ts";
import { actingView } from "./acting.test-support.ts";
import { actsFor, assignOwner, bulkPlan, doneMutations, factsOf, evidenceDraft, hasArtifact, hasNote, missingEvidence, runBulk, selectionOf, type Act, type BulkAction, type Facts } from "./acts.ts";

const view = actingView();

function facts(key: string, change: Partial<Facts> = {}): Facts {
  const found = factsOf(view, key);
  if (found === undefined) {
    throw new Error(`no node ${key}`);
  }
  return { ...found, ...change };
}

describe("C11: each kind's actions", () => {
  const derivedOf = (key: string) => facts(key).derived;
  const cases: [string, Facts, Act[]][] = [
    ["a decision", facts("n_pick"), ["answer", "skip", "snooze"]],
    ["a deliverable to do", facts("n_work"), ["start", "done", "snooze", "skip", "canvas"]],
    ["a deliverable under way", facts("n_work", { state: "active" }), ["done", "snooze", "skip", "canvas"]],
    ["a milestone", facts("n_meet"), ["reach", "skip", "snooze", "canvas"]],
    ["a placeholder to break down", facts("n_hold"), ["breakdown", "atomic", "snooze"]],
    ["a placeholder marked atomic", facts("n_hold", { derived: { ...derivedOf("n_hold"), needs_breakdown: false } }), ["start", "done", "snooze", "skip", "canvas"]],
  ];
  it.each(cases)("%s", (_, given, expected) => {
    expect(actsFor(given)).toEqual(expected);
  });

  it("a placeholder still to break down has no done (B10)", () => {
    expect(actsFor(facts("n_hold"))).not.toContain("done");
  });
});

describe("C11 done with its artifact (G2) or note (G4)", () => {
  it("links the artifact before completing, in one patch, so the guard sees it", () => {
    const mutations = doneMutations("n_work", { artifact: { key: "a_new", url: "https://example.org/out" } });
    expect(mutations.map((mutation) => mutation.op)).toEqual(["add_annotation", "transition"]);
    expect(mutations[0]).toMatchObject({ annotation: { key: "a_new", node: "n_work", artifact: "https://example.org/out" } });
  });

  it("adds the note before completing, in one patch, and both evidences when both are needed", () => {
    const note = { key: "a_note", text: "Covered the upgrade path." };
    expect(doneMutations("n_log", { note }).map((mutation) => mutation.op)).toEqual(["add_annotation", "transition"]);
    expect(doneMutations("n_log", { note })[0]).toMatchObject({ annotation: { key: "a_note", node: "n_log", note: "Covered the upgrade path." } });
    expect(doneMutations("n_work", { artifact: { key: "a_new", url: "https://example.org/out" }, note }).map((mutation) => mutation.op)).toEqual(["add_annotation", "add_annotation", "transition"]);
  });

  it("reads an unsent form saved before notes as its artifact address", () => {
    expect(evidenceDraft("https://example.org/out")).toEqual({ artifact: "https://example.org/out", note: "" });
    expect(evidenceDraft({ artifact: "", note: "Written up." })).toEqual({ artifact: "", note: "Written up." });
  });

  it("asks only for the evidence a node requires and lacks; a link or another node's note is not a note", () => {
    expect(hasArtifact(view, "n_work")).toBe(false);
    expect(missingEvidence(view, facts("n_work").node)).toEqual({ artifact: true, note: false });
    expect(missingEvidence(view, facts("n_log").node)).toEqual({ artifact: false, note: true });
    const state = view.journey.graph.state ?? {};
    const annotated = (annotations: NonNullable<typeof state.annotations>): typeof view => ({ ...view, journey: { ...view.journey, graph: { ...view.journey.graph, state: { ...state, annotations } } } });
    const others = annotated([
      { key: "a_link", node: "n_log", reference: "https://example.org/r" },
      { key: "a_journey", note: "On the journey." },
      { key: "a_other", node: "n_work", note: "On another node." },
    ].map((body) => ({ body, created_by: "u_one", created_at: "2026-10-01T00:00:00Z" })));
    expect(hasNote(others, "n_log")).toBe(false);
    expect(missingEvidence(others, facts("n_log").node).note).toBe(true);
    const own = annotated([{ body: { key: "a_own", node: "n_log", note: "Written up." }, created_by: "u_one", created_at: "2026-10-01T00:00:00Z" }]);
    expect(missingEvidence(own, facts("n_log").node).note).toBe(false);
  });
});

describe("D2: assign owner", () => {
  it("sets the owner explicitly", () => {
    expect(assignOwner("n_pick", "e_one")).toEqual({ op: "set_participation", node: "n_pick", kind: "k_owner", source: ["e_one"] });
  });
});

describe("C9: bulk actions", () => {
  const work = [facts("n_work"), facts("n_hold")];

  it("acts on every node selected, on whatever page, in the order selected; a node gone since is dropped", () => {
    expect(selectionOf(view, ["n_hold", "n_gone_since", "n_meet"]).map((each) => each.node.key)).toEqual(["n_hold", "n_meet"]);
  });

  it("makes one mutation per node, in the order selected", () => {
    const plan = bulkPlan({ act: "done" }, [facts("n_meet"), ...work]);
    expect(plan).toEqual({
      mutations: [
        { op: "transition", node: "n_meet", transition: "reach" },
        { op: "transition", node: "n_work", transition: "complete" },
        { op: "transition", node: "n_hold", transition: "complete" },
      ],
    });
  });

  const cases: [string, BulkAction, Facts[], string[]][] = [
    ["start a decision", { act: "start" }, [facts("n_work"), facts("n_pick")], ["n_pick"]],
    ["complete a decision", { act: "done" }, [facts("n_pick")], ["n_pick"]],
    ["snooze a finished node", { act: "snooze", until: { date: "2026-10-09" } }, [facts("n_pick"), facts("n_done")], ["n_done"]],
    ["unsnooze a node with no snooze", { act: "unsnooze" }, [facts("n_wait"), facts("n_pick")], ["n_pick"]],
  ];
  it.each(cases)("sends nothing when one cannot take it: %s", (_, action, selected, unable) => {
    expect(bulkPlan(action, selected)).toEqual({ unable });
  });

  it("skips each with the one reason", () => {
    const plan = bulkPlan({ act: "skip", reason: "not needed" }, work);
    expect("mutations" in plan ? plan.mutations.map((mutation) => mutation.op === "transition" && mutation.transition) : []).toEqual([
      { skip: { reason: "not needed" } },
      { skip: { reason: "not needed" } },
    ]);
  });

  /** A fake write path: each patch it is asked to send, and whether each lands. */
  function recorder() {
    const sent: Mutation[][] = [];
    const run = (patch: Mutation[]) => {
      sent.push(patch);
      return Promise.resolve(true);
    };
    return { sent, run };
  }

  it("sends the whole action as one patch", async () => {
    const { sent, run } = recorder();
    const outcome = await runBulk({ act: "snooze", until: { node: "n_meet" } }, work, run);
    expect(outcome).toEqual({ outcome: "sent", landed: true });
    expect(sent).toHaveLength(1);
    expect(sent[0]?.map((mutation) => mutation.op === "snooze" && mutation.node)).toEqual(["n_work", "n_hold"]);
  });

  it("sends nothing when a node cannot take the action", async () => {
    const { sent, run } = recorder();
    expect(await runBulk({ act: "start" }, [facts("n_work"), facts("n_pick")], run)).toEqual({ outcome: "unable", unable: ["n_pick"] });
    expect(sent).toEqual([]);
  });
});
