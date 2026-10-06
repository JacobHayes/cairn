// H5: the safe retry, case for case with the Rust client's (crates/api/src/client/retry.rs),
// the H5 fix's "never rebase backward" included.
import { describe, expect, it } from "vitest";

import {
  RESUBMISSION_COUNT_MAX,
  rebased,
  submit,
  type Answered,
  type Overlaps,
  type Patch,
  type RevisionConflict,
  type TouchedSet,
} from "./retry.ts";

type Of = RevisionConflict["of"];

function patch(deploymentRevision?: number): Patch {
  const base: Patch = {
    id: "p_one",
    target: { journey: "j_one" },
    base_revision: 2,
    mutations: [{ op: "transition", node: "n_a", transition: "start" }],
  };
  return deploymentRevision === undefined ? base : { ...base, deployment_revision: deploymentRevision };
}

const journey = (id: string): Of => ({ domain: { journey: id } });
const deployment: Of = { domain: "deployment" };
const conflict = (of: Of, expected: number, current: number): RevisionConflict => ({ of, expected, current });

/** What touches node `node` of journey `j_one` alone. */
const onNode = (node: string): TouchedSet => [{ in_graph: { graph: { journey: "j_one" }, key: { node } } }];

/** The engine's overlap check, for a patch that touches node `n_a` alone. */
const overlaps: Overlaps = (_patch, intervening) => JSON.stringify(intervening).includes('"n_a"');

describe("rebased", () => {
  it("moves a patch onto the revisions the rejection reports, keeping its id and mutations", () => {
    const original = patch(1);
    const conflicts = [conflict(journey("j_one"), 2, 4), conflict(deployment, 1, 3)];
    const retry = rebased(original, conflicts, onNode("n_b"), overlaps);
    expect(retry).toEqual({ ...original, base_revision: 4, deployment_revision: 3 });
  });

  it.each<[string, RevisionConflict[], TouchedSet]>([
    ["what intervened touched the node", [conflict(journey("j_one"), 2, 3)], onNode("n_a")],
    ["nothing moved", [], onNode("n_b")],
    ["another journey moved", [conflict(journey("j_two"), 1, 2)], onNode("n_b")],
    ["the patch names no deployment revision to move", [conflict(deployment, 1, 2)], onNode("n_b")],
    ["a proposal's revision moved", [conflict({ proposal: "q_one" }, 1, 2)], onNode("n_b")],
    ["the patch names a revision past the current one", [conflict(journey("j_one"), 2, 1)], []],
    ["the revision the patch names has not moved", [conflict(journey("j_one"), 2, 2)], []],
  ])("is none when %s", (_why, conflicts, intervening) => {
    expect(rebased(patch(), conflicts, intervening, overlaps)).toBeUndefined();
  });

  it("is none when the engine cannot say whether the sets overlap", () => {
    const refusing: Overlaps = () => {
      throw new Error("version skew");
    };
    expect(rebased(patch(), [conflict(journey("j_one"), 2, 3)], onNode("n_b"), refusing)).toBeUndefined();
  });
});

/** Answers each submission in turn: a stale answer with the given intervening set, or a landing. */
function scripted(answers: (TouchedSet | undefined)[]) {
  const sent: number[] = [];
  const send = (submitted: Patch): Promise<Answered<never>> => {
    sent.push(submitted.base_revision);
    const intervening = answers[sent.length - 1];
    if (intervening === undefined) {
      const receipt = { patch_id: submitted.id, domain: { journey: "j_one" }, content_hash: "h", revision: submitted.base_revision + 1 };
      return Promise.resolve({ outcome: "answered", answer: { outcome: "already_applied", receipt } });
    }
    const conflicts = [conflict(journey("j_one"), submitted.base_revision, submitted.base_revision + 1)];
    return Promise.resolve({ outcome: "rejected", rejection: { rejection: "stale", conflicts, intervening } });
  };
  return { sent, send };
}

describe("submit", () => {
  it("resubmits while a stale answer is safe, and lands", async () => {
    const { sent, send } = scripted([onNode("n_b"), undefined]);
    const submitted = await submit(patch(), send, { overlaps });
    expect(submitted).toMatchObject({ outcome: "landed", resubmitted: 1 });
    expect(sent).toEqual([2, 3]);
  });

  it("surfaces a stale answer that is not safe", async () => {
    const { sent, send } = scripted([onNode("n_a")]);
    const submitted = await submit(patch(), send, { overlaps });
    expect(submitted).toMatchObject({ outcome: "rejected", rejection: { rejection: "stale" } });
    expect(sent).toEqual([2]);
  });

  it("gives up after the resubmission bound", async () => {
    const { sent, send } = scripted(Array.from({ length: RESUBMISSION_COUNT_MAX + 1 }, () => onNode("n_b")));
    const submitted = await submit(patch(), send, { overlaps });
    expect(submitted.outcome).toBe("rejected");
    expect(sent).toHaveLength(RESUBMISSION_COUNT_MAX + 1);
  });

  it("stops retrying the moment retries are no longer allowed (version skew)", async () => {
    const { sent, send } = scripted([onNode("n_b"), onNode("n_b"), undefined]);
    let allowed = true;
    const submitted = await submit(patch(), (next) => {
      const answered = send(next);
      allowed = sent.length < 2;
      return answered;
    }, { overlaps, mayRetry: () => allowed });
    expect(submitted).toMatchObject({ outcome: "rejected", resubmitted: 1 });
    expect(sent).toEqual([2, 3]);
  });

  /** 6.1's finding: a stale answer naming a revision in flight, then one naming a revision
   * behind it. The client rebases forward once and surfaces the second, never going back. */
  it("never rebases backward", async () => {
    const sent: number[] = [];
    const submitted = await submit(patch(), (next) => {
      const named = next.base_revision;
      sent.push(named);
      const current = named === 2 ? 3 : 2;
      const rejection = { rejection: "stale" as const, conflicts: [conflict(journey("j_one"), named, current)], intervening: [] };
      return Promise.resolve<Answered<never>>({ outcome: "rejected", rejection });
    }, { overlaps });
    expect(submitted.outcome).toBe("rejected");
    expect(sent).toEqual([2, 3]);
  });

  it("passes a failure through without retrying", async () => {
    const submitted = await submit(patch(), () => Promise.resolve<Answered<string>>({ outcome: "failed", error: "down" }), { overlaps });
    expect(submitted).toEqual({ outcome: "failed", error: "down", resubmitted: 0 });
  });
});
