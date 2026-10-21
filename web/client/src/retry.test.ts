// H5: the safe retry's loop. Whether a stale patch is safe to resubmit, and as what, is the
// engine's rule (the schema crate's tests, and the browser host's through the module); here
// it is a stand-in, and the tests are of what the loop does with its answers.
import { describe, expect, it } from "vitest";

import {
  RESUBMISSION_COUNT_MAX,
  submit,
  type Answered,
  type Patch,
  type Rebase,
  type RevisionConflict,
  type TouchedSet,
} from "./retry.ts";

type Of = RevisionConflict["of"];

const patch: Patch = {
  id: "p_one",
  target: { journey: "j_one" },
  base_revision: 2,
  mutations: [{ op: "transition", node: "n_a", transition: "start" }],
};

const journey = (id: string): Of => ({ domain: { journey: id } });
const conflict = (of: Of, expected: number, current: number): RevisionConflict => ({ of, expected, current });

/** What touches node `node` of journey `j_one` alone. */
const onNode = (node: string): TouchedSet => [{ in_graph: { graph: { journey: "j_one" }, key: { node } } }];

/** A stand-in for the engine's rule, for a patch that touches node `n_a` alone: onto the first conflict's revision unless what intervened touched it. */
const rebase: Rebase = (submitted, { conflicts, intervening }) =>
  Promise.resolve(JSON.stringify(intervening).includes('"n_a"') ? undefined : { ...submitted, base_revision: conflicts[0]?.current ?? submitted.base_revision });

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
    return Promise.resolve({ outcome: "rejected", rejection: { rejection: "stale", conflicts, intervening, touched: onNode("n_a") } });
  };
  return { sent, send };
}

describe("submit", () => {
  it("resubmits while a stale answer is safe, and lands", async () => {
    const { sent, send } = scripted([onNode("n_b"), undefined]);
    const submitted = await submit(patch, send, { rebase });
    expect(submitted).toMatchObject({ outcome: "landed", resubmitted: 1 });
    expect(sent).toEqual([2, 3]);
  });

  it("surfaces a stale answer that is not safe", async () => {
    const { sent, send } = scripted([onNode("n_a")]);
    const submitted = await submit(patch, send, { rebase });
    expect(submitted).toMatchObject({ outcome: "rejected", rejection: { rejection: "stale" } });
    expect(sent).toEqual([2]);
  });

  it("gives up after the resubmission bound", async () => {
    const { sent, send } = scripted(Array.from({ length: RESUBMISSION_COUNT_MAX + 1 }, () => onNode("n_b")));
    const submitted = await submit(patch, send, { rebase });
    expect(submitted.outcome).toBe("rejected");
    expect(sent).toHaveLength(RESUBMISSION_COUNT_MAX + 1);
  });

  it("stops retrying the moment retries are no longer allowed (version skew)", async () => {
    const { sent, send } = scripted([onNode("n_b"), onNode("n_b"), undefined]);
    let allowed = true;
    const submitted = await submit(patch, (next) => {
      const answered = send(next);
      allowed = sent.length < 2;
      return answered;
    }, { rebase, mayRetry: () => allowed });
    expect(submitted).toMatchObject({ outcome: "rejected", resubmitted: 1 });
    expect(sent).toEqual([2, 3]);
  });

  it("sends nothing more when retries stop while the engine is asked (version skew)", async () => {
    const { sent, send } = scripted([onNode("n_b"), undefined]);
    let allowed = true;
    const slowRebase: Rebase = async (...stale) => {
      allowed = false;
      return rebase(...stale);
    };
    const submitted = await submit(patch, send, { rebase: slowRebase, mayRetry: () => allowed });
    expect(submitted).toMatchObject({ outcome: "rejected", resubmitted: 0 });
    expect(sent).toEqual([2]);
  });

  it("surfaces a stale answer the engine refused to rebase", async () => {
    const { sent, send } = scripted([onNode("n_b")]);
    const submitted = await submit(patch, send, { rebase: () => Promise.reject(new Error("the module stopped")) });
    expect(submitted).toMatchObject({ outcome: "rejected", resubmitted: 0 });
    expect(sent).toEqual([2]);
  });

  it("passes a failure through without retrying", async () => {
    const submitted = await submit(patch, () => Promise.resolve<Answered<string>>({ outcome: "failed", error: "down" }), { rebase });
    expect(submitted).toEqual({ outcome: "failed", error: "down", resubmitted: 0 });
  });
});
