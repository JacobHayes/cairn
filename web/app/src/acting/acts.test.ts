// C11 done with its inline artifact (G2), D2's inline assign, and C9's bulk actions: one patch,
// one mutation per node, nothing sent when a selected node cannot take the action.
import { describe, expect, it } from "vitest";

import type { Mutation } from "../detail/model.ts";
import { actingView } from "./acting.test-support.ts";
import { assignOwner, bulkPlan, doneMutations, evidenceComplete, factsOf, missingEvidence, reasonFor, runBulk, selectionOf, type BulkAction, type Facts } from "./acts.ts";

const view = actingView();

function facts(key: string, change: Partial<Facts> = {}): Facts {
  const found = factsOf(view, key);
  if (found === undefined) {
    throw new Error(`no node ${key}`);
  }
  return { ...found, ...change };
}

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

  it("asks only for the evidence the engine says is missing", () => {
    expect(missingEvidence(view, facts("n_work").node)).toEqual({ artifact: true, note: false });
    expect(missingEvidence(view, facts("n_log").node)).toEqual({ artifact: false, note: true });
    expect(missingEvidence(view, facts("n_later").node)).toEqual({ artifact: false, note: false });
  });
});

describe("C11 the done form (G2, G4)", () => {
  it("waits for each piece of evidence the node needs, and takes none it does not", () => {
    const both = { artifact: true, note: true };
    expect(evidenceComplete({ artifact: false, note: true }, { artifact: "", note: "  " })).toBe(false);
    expect(evidenceComplete({ artifact: false, note: true }, { artifact: "", note: "Covered it." })).toBe(true);
    expect(evidenceComplete(both, { artifact: "https://example.org/out", note: "" })).toBe(false);
    expect(evidenceComplete(both, { artifact: "https://example.org/out", note: "Covered it." })).toBe(true);
  });
});

describe("D2: assign owner", () => {
  it("sets the owner explicitly", () => {
    expect(assignOwner("n_pick", "who_one")).toEqual({ op: "set_participation", node: "n_pick", kind: "k_owner", source: ["who_one"] });
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

  it("snoozes a container like any node, and one snoozed only through its container is not unsnoozed alone", () => {
    expect(bulkPlan({ act: "snooze", until: { node: "n_meet" } }, [facts("n_stage")])).toEqual({
      mutations: [{ op: "snooze", node: "n_stage", until: { node: "n_meet" } }],
    });
    const held = facts("n_work", { derived: { ...facts("n_work").derived, snoozed: { node: "n_meet" }, snoozed_via: "n_stage" } });
    expect(bulkPlan({ act: "unsnooze" }, [held])).toEqual({ unable: ["n_work"] });
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

it("an action is offered over a selection only when every member can take it, and says who cannot", () => {
  const title = (key: string) => key.slice(2);
  expect(reasonFor({ act: "start" }, [facts("n_work")], title)).toBeUndefined();
  const one = reasonFor({ act: "start" }, [facts("n_work"), facts("n_pick")], title) ?? "";
  expect(one).toContain("pick");
  expect(one).not.toContain("work");
  const two = reasonFor({ act: "done" }, [facts("n_pick"), facts("n_pick")], title) ?? "";
  expect(two).toContain("pick and pick");
});
