// The shared write path: H5's safe retry through the host with a fresh patch id, D7's
// consequences noticed, and nothing sent under version skew.
import { describe, expect, it } from "vitest";

import { FakeHost } from "./fake.test-support.ts";
import { Notices, consequenceLines } from "./notices.ts";
import { SkewLatch } from "./skew.ts";
import { newPatchId, patchOf, write, type WriteIntent } from "./writes.ts";

const intent: WriteIntent = {
  target: { journey: "j_one" },
  baseRevision: 3,
  mutations: [{ op: "set_node_field", node: "n_a", value: { title: "Renamed" } }],
};

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };

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

  it("retries a safe stale answer on the reported revision, lands, and notices what it caused", async () => {
    const host = new FakeHost();
    const notices = new Notices();
    const conflicts = [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 5 }];
    host.answers = [
      { outcome: "rejected", rejection: { rejection: "stale", conflicts, intervening: [] } },
      { outcome: "answered", answer: { outcome: "applied", receipt, consequences: { j_one: { overdue: ["n_b"] } } } },
    ];
    const result = await write(host, new SkewLatch(), notices, intent);
    expect(result).toMatchObject({ outcome: "landed", resubmitted: 1 });
    expect(host.sent.map((patch) => patch.base_revision)).toEqual([3, 5]);
    expect(new Set(host.sent.map((patch) => patch.id)).size).toBe(1);
    expect(notices.current.map((notice) => notice.lines)).toEqual([[{ kind: "overdue", journey: "j_one", nodes: ["n_b"] }]]);
  });

  it("sends nothing once the tab is in version skew", async () => {
    const host = new FakeHost();
    const skew = new SkewLatch();
    skew.latch({ document: "2.0.0", engine: "1.0.0" });
    expect(await write(host, skew, new Notices(), intent)).toEqual({ outcome: "stopped" });
    expect(host.sent).toEqual([]);
  });

  it("notices a failure, and returns it", async () => {
    const host = new FakeHost();
    const notices = new Notices();
    host.answers = [{ outcome: "failed", error: { status: 503, message: "busy" } }];
    expect(await write(host, new SkewLatch(), notices, intent)).toEqual({ outcome: "failed", failure: { status: 503, message: "busy" } });
    expect(notices.current.map((notice) => notice.tone)).toEqual(["problem"]);
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
});
