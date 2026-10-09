// The shared write path: H5's safe retry through the host with a fresh patch id, D7's
// consequences recorded as a save with its warning, what the sync chip says about a write
// that did not land, and nothing sent under version skew.
import { describe, expect, it } from "vitest";

import { Activity, consequenceLines, warningOf } from "./activity.ts";
import { FakeHost } from "./fake.test-support.ts";
import { SkewLatch } from "./skew.ts";
import { SyncStatus } from "./sync.ts";
import { newPatchId, patchOf, unlocksOf, write, type WriteEnv, type WriteIntent } from "./writes.ts";

const intent: WriteIntent = {
  target: { journey: "j_one" },
  baseRevision: 3,
  mutations: [{ op: "set_node_field", node: "n_a", value: { title: "Renamed" } }],
};

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };

const titles: Record<string, string> = { n_a: "Draft the plan", n_b: "Kickoff" };

function envOf(host: FakeHost, skew = new SkewLatch()): WriteEnv {
  const timers = { set: () => 0, clear: () => undefined };
  return { host, skew, activity: new Activity(), sync: new SyncStatus(timers, () => 0, false), titleOf: (_journey, node) => titles[node] ?? node };
}

describe("write", () => {
  it("sends a patch drafted against the revision its author saw, with a fresh id", () => {
    const patch = patchOf(intent);
    expect(patch).toMatchObject({ target: { journey: "j_one" }, base_revision: 3, mutations: intent.mutations });
    expect(patch).not.toHaveProperty("deployment_revision");
    expect(patch.id).toMatch(/^p_[a-z0-9][a-z0-9_-]*$/);
    expect(patch.id.length).toBeLessThanOrEqual(64);
    expect(newPatchId()).not.toBe(newPatchId());
    expect(patchOf({ ...intent, deploymentRevision: 2 })).toMatchObject({ deployment_revision: 2 });
  });

  it("retries a safe stale answer on the reported revision, lands, and records the save with its warning", async () => {
    const host = new FakeHost();
    const env = envOf(host);
    const conflicts = [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 5 }];
    host.answers = [
      { outcome: "rejected", rejection: { rejection: "stale", conflicts, intervening: [] } },
      { outcome: "answered", answer: { outcome: "applied", receipt, consequences: { j_one: { overdue: ["n_b"] } } } },
    ];
    const result = await write(env, intent);
    expect(result).toMatchObject({ outcome: "landed", resubmitted: 1, warning: "Kickoff is now overdue." });
    expect(host.sent.map((patch) => patch.base_revision)).toEqual([3, 5]);
    expect(new Set(host.sent.map((patch) => patch.id)).size).toBe(1);
    expect(env.activity.saves.map(({ text, warning }) => ({ text, warning }))).toEqual([{ text: "Edited Draft the plan", warning: "Kickoff is now overdue." }]);
    expect(env.sync.summary.state).toBe("saved");
  });

  it("sends nothing once the tab is in version skew, and the chip says the edit was not sent", async () => {
    const host = new FakeHost();
    const skew = new SkewLatch();
    skew.latch({ document: "2.0.0", engine: "1.0.0" });
    const env = envOf(host, skew);
    expect(await write(env, intent)).toEqual({ outcome: "stopped" });
    expect(host.sent).toEqual([]);
    expect(env.sync.summary.state).toBe("not-saved");
    expect(env.sync.problems.map((each) => each.message)).toEqual([expect.stringContaining("Reload")]);
  });

  it("keeps a write that got no answer on the chip until the next one to that target lands", async () => {
    const host = new FakeHost();
    const env = envOf(host);
    host.answers = [{ outcome: "failed", error: { status: 503, message: "busy" } }];
    expect(await write(env, intent)).toEqual({ outcome: "failed", failure: { status: 503, message: "busy" } });
    expect(env.sync.summary).toMatchObject({ state: "not-saved", tone: "act" });
    expect(env.sync.problems.map((problem) => problem.message)).toEqual(["Could not confirm the save: busy"]);
    host.answers = [{ outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } }];
    await write(env, intent);
    expect(env.sync.problems).toEqual([]);
  });

});

describe("unlocksOf (D7)", () => {
  const answer = (unlocked: string[]) => ({ outcome: "applied" as const, receipt, consequences: { j_one: { unlocked } } });
  const complete = { op: "transition" as const, node: "n_a", transition: "complete" as const };

  it("names the node acted on even when an evidence annotation comes first, and when nothing was unlocked", () => {
    const annotate = { op: "add_annotation" as const, annotation: { key: "a_x", node: "n_a", note: "Done" } };
    expect(unlocksOf({ ...intent, mutations: [annotate, complete] }, answer(["n_b"]), 7)).toEqual({ journey: "j_one", by: "n_a", nodes: ["n_b"], began: 7 });
    expect(unlocksOf({ ...intent, mutations: [complete] }, answer([]), 8)).toMatchObject({ by: "n_a", nodes: [] });
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
