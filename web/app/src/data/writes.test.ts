// What a write is made of: H5's patch with a fresh id, and D7's consequences recorded as a save
// with its warning. The delivery is write-machine.test.ts's.
import { describe, expect, it } from "vitest";

import { consequenceLines, warningOf } from "./activity.ts";
import { newPatchId, patchOf, unlockedOf, type WriteIntent } from "./writes.ts";

const intent: WriteIntent = {
  target: { journey: "j_one" },
  baseRevision: 3,
  mutations: [{ op: "set_node_field", node: "n_a", value: { title: "Renamed" } }],
};

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };

const titles: Record<string, string> = { n_a: "Draft the plan", n_b: "Kickoff" };

describe("patchOf", () => {
  it("drafts a patch against the revision its author saw, with a fresh id", () => {
    const patch = patchOf(intent);
    expect(patch).toMatchObject({ target: { journey: "j_one" }, base_revision: 3, mutations: intent.mutations });
    expect(patch).not.toHaveProperty("deployment_revision");
    expect(patch.id).toMatch(/^p_[a-z0-9][a-z0-9_-]*$/);
    expect(patch.id.length).toBeLessThanOrEqual(64);
    expect(newPatchId()).not.toBe(newPatchId());
    expect(patchOf({ ...intent, deploymentRevision: 2 })).toMatchObject({ deployment_revision: 2 });
  });
});

describe("unlockedOf (D7)", () => {
  const answer = (unlocked: string[]) => ({ outcome: "applied" as const, receipt, consequences: { j_one: { unlocked } } });
  const complete = { op: "transition" as const, node: "n_a", transition: "complete" as const };

  it("names the node acted on even when an evidence annotation comes first, and when nothing was unlocked", () => {
    const annotate = { op: "add_annotation" as const, annotation: { key: "a_x", node: "n_a", note: "Done" } };
    expect(unlockedOf({ ...intent, mutations: [annotate, complete] }, answer(["n_b"]), 7)).toEqual({ journey: "j_one", by: "n_a", nodes: ["n_b"], began: 7 });
    expect(unlockedOf({ ...intent, mutations: [complete] }, answer([]), 8)).toMatchObject({ by: "n_a", nodes: [] });
  });
});

describe("warningOf (D7)", () => {
  const titleOf = (_journey: string, node: string) => titles[node] ?? node;
  it("says one sentence per warning, nothing when there is none, and counts the rest", () => {
    expect(warningOf([], titleOf)).toBeUndefined();
    expect(warningOf([{ kind: "overdue", journey: "j_one", nodes: ["n_b"] }], titleOf)).toBe("Kickoff is now overdue.");
    expect(warningOf([{ kind: "stale", journey: "j_one", nodes: ["n_a", "n_b", "n_c"] }], titleOf)).toBe("Draft the plan and 2 more are now stale.");
  });
});

describe("consequenceLines (D7)", () => {
  it("lists each kind a journey newly has, and nothing for a resubmission's receipt", () => {
    const answer = {
      outcome: "applied" as const,
      receipt,
      consequences: {
        j_one: {
          stale: [{ node: "n_a", reasons: [] }],
          shortfalls: [],
          stalled: { waiting_on: [] },
        },
      },
    };
    expect(consequenceLines(answer)).toEqual([
      { kind: "stale", journey: "j_one", nodes: ["n_a"] },
      { kind: "stalled", journey: "j_one", nodes: [] },
    ]);
    expect(consequenceLines({ outcome: "already_applied", receipt })).toEqual([]);
  });

  it("lists a route write's unanchored notices by path (A20)", () => {
    const notice = { code: "unanchored" as const, node: "n_a", path: "setup/a", message: "a has no chain" };
    const answer = { outcome: "applied" as const, receipt, consequences: {}, notices: [notice] };
    expect(consequenceLines(answer)).toEqual([{ kind: "unanchored", journey: "", nodes: ["setup/a"] }]);
  });

  it("lists finished work that may not apply with every decision it waits on, each once (D4)", () => {
    const answer = {
      outcome: "applied" as const,
      receipt,
      consequences: {
        j_one: {
          undecided: [
            { node: "n_a", unanswered: ["n_flag"] },
            { node: "n_b", unanswered: ["n_other", "n_flag"] },
          ],
        },
      },
    };
    expect(consequenceLines(answer)).toEqual([{ kind: "undecided", journey: "j_one", nodes: ["n_a", "n_b"], unanswered: ["n_flag", "n_other"] }]);
  });
});
