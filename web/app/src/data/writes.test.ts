// What a write is made of: H5's patch with a fresh id, and D7's consequences recorded as a save
// with its warning. The delivery is write-machine.test.ts's.
import { KEY_PREFIX } from "@cairn/client";
import { describe, expect, it } from "vitest";


import { consequenceLines, warningOf } from "./activity.ts";
import type { Command } from "./edit-command.ts";
import { landedWarning, newPatchId, patchOf, saveOf, subjectOf, unlockedOf, type Mutation, type WriteIntent } from "./writes.ts";

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
    expect(patch.id.startsWith(KEY_PREFIX.PatchId)).toBe(true);
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
    expect(warningOf([{ kind: "stale", journey: "j_one", nodes: ["n_a", "n_b", "n_c"] }], titleOf)).toBe("Draft the plan, Kickoff and n_c are now stale.");
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

describe("saveOf", () => {
  const titleOf = (_journey: string, node: string) => titles[node] ?? node;
  const applied = { outcome: "applied" as const, receipt, consequences: { j_one: { overdue: ["n_b"] } } };
  const call = (op: "apply" | "discard"): Command => ({
    kind: "proposal",
    call: op === "apply" ? { op, destination: { journey: "j_one" }, proposal: "pr_one", request: { patch_id: "p_x", reviewed_revision: 4 } } : { op, destination: { journey: "j_one" }, proposal: "pr_one", request: { patch_id: "p_x", base_revision: 4 } },
  });

  it("records a patch as what it did, with the consequences and the work it unlocked", () => {
    const patch = patchOf(intent, "p_x");
    const save = saveOf({ kind: "patch", patch }, applied, 5, titleOf);
    expect(save?.text).toContain("Draft the plan");
    expect(save?.lines).toEqual(consequenceLines(applied));
    expect(save?.unlocked).toEqual(unlockedOf(intent, applied, 5));
  });

  it("records an imported file and an applied proposal, with their consequences", () => {
    const file = { format: 1, route: "r_one", name: "One", nodes: [] } as never;
    expect(saveOf({ kind: "import", request: { patch_id: "p_x", file } }, applied, 5, titleOf)).toMatchObject({ text: expect.stringContaining("One") as string, lines: consequenceLines(applied) });
    expect(saveOf(call("apply"), applied, 5, titleOf)).toMatchObject({ lines: consequenceLines(applied) });
  });

  it("records the drafting of a proposal with the way to it, whichever way it was drafted, so it stays reachable", () => {
    const drafting: [Command, string][] = [
      [{ kind: "proposal", call: { op: "create", destination: { journey: "j_one" }, request: { id: "pr_new", patch_id: "p_x", draft: {} as never } } }, "/journeys/j_one/proposals/pr_new"],
      [{ kind: "proposal", call: { op: "upgrade", journey: "j_one", request: { proposal: "pr_new", patch_id: "p_x", to: 2 } } }, "/journeys/j_one/proposals/pr_new"],
      // Saving a journey as a route drafts a proposal for the route, which does not exist yet.
      [{ kind: "proposal", call: { op: "save_as_route", journey: "j_one", request: { proposal: "pr_new", patch_id: "p_x", route: "r", name: "R" } } }, "/routes/r/proposals/pr_new"],
      [{ kind: "proposal", call: { op: "relink", journey: "j_one", request: { proposal: "pr_new", patch_id: "p_x", route: "r", version: 2 } as never } }, "/journeys/j_one/proposals/pr_new"],
    ];
    for (const [command, address] of drafting) {
      expect(saveOf(command, { outcome: "already_saved" } as never, 5, titleOf)?.address).toBe(address);
    }
  });

  it("records nothing for the proposal writes that change nothing yet, nor for an apply answered as a proposal", () => {
    expect(saveOf(call("discard"), applied, 5, titleOf)).toBeUndefined();
    expect(saveOf(call("discard"), { outcome: "already_saved" } as never, 5, titleOf)).toBeUndefined();
    expect(saveOf(call("apply"), { outcome: "already_saved" } as never, 5, titleOf)).toBeUndefined();
  });
});

describe("landedWarning", () => {
  const titleOf = (_journey: string, node: string) => titles[node] ?? node;
  it("is the warning of a patch's consequences, and none for a proposal's answer", () => {
    expect(landedWarning({ outcome: "applied", receipt, consequences: { j_one: { overdue: ["n_b"] } } }, titleOf)).toBe("Kickoff is now overdue.");
    expect(landedWarning({ outcome: "already_saved" } as never, titleOf)).toBeUndefined();
  });
});

describe("subjectOf", () => {
  it("names the node every kind of mutation is about", () => {
    const node = { key: "n_new", id: "new", kind: "action", title: "New" } as Extract<Mutation, { op: "add_node" }>["node"];
    const cases: [Mutation, string][] = [
      [{ op: "add_node", node }, "n_new"],
      [{ op: "remove_node", removal: { node: "n_old" } }, "n_old"],
      [{ op: "add_edge", edge: { node: "n_a", requires: "n_b" } }, "n_a"],
      [{ op: "remove_edge", edge: { node: "n_c", requires: "n_b" } }, "n_c"],
      [{ op: "answer", decision: "n_d", value: { boolean: true } }, "n_d"],
      [{ op: "set_pin", node: "n_e", date: "2026-11-01" }, "n_e"],
    ];
    expect(cases.map(([mutation]) => subjectOf(mutation))).toEqual(cases.map(([, expected]) => expected));
  });
});
