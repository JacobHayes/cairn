// E6: a merge reads what refers to each identity it takes in, then delivers one deployment patch
// naming every journey at the revision read; the write is the edit manager's, so stopping the
// flow never stops it.
import type { Answered, HttpFailure } from "@cairn/client";
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { startRoot } from "../data/edits.test-support.ts";
import { deferred, settled } from "../data/fake.test-support.ts";
import type { IdentityReferences } from "../data/host.ts";
import { mergeFlow, type Merge, type Merged } from "./merge.ts";

const merge: Merge = { survivor: "who_keep", merged: ["who_old"], deployment: 7 };
const referencing = (...journeys: [string, number][]): IdentityReferences => ({ journeys: journeys.map(([id, revision]) => ({ id, revision })) as IdentityReferences["journeys"], routes: [] });
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt: { patch_id: "p_x", domain: "deployment", content_hash: "h", revision: 8 }, consequences: {} } };
const rejected: Answered<HttpFailure> = { outcome: "rejected", rejection: { rejection: "invalid", violations: [] } };

async function started(references: (identity: string) => Promise<IdentityReferences>) {
  const root = await startRoot();
  const flow = createActor(mergeFlow, { input: { edits: root.edits, key: "merge-identities", references } });
  const merged: Merged[] = [];
  flow.on("merged", (each) => merged.push(each));
  flow.start();
  return { ...root, flow, merged };
}

describe("a merge", () => {
  it("names every journey read for each identity taken in, at the revision read, in a deployment patch against the deployment revision asked", async () => {
    const read: string[] = [];
    const { flow, host, merged } = await started((identity) => {
      read.push(identity);
      return Promise.resolve(identity === "who_keep" ? referencing(["j_one", 4]) : referencing(["j_one", 4], ["j_two", 9]));
    });
    host.answers = [applied];
    flow.send({ type: "merge", ...merge });
    await settled();
    expect(read).toEqual(["who_keep", "who_old"]);
    expect(host.sent).toHaveLength(1);
    expect(host.sent[0]).toMatchObject({
      target: "deployment",
      base_revision: 7,
      mutations: [{ op: "merge_identities", survivor: "who_keep", merged: ["who_old"], journeys: { j_one: 4, j_two: 9 } }],
    });
    expect(merged).toHaveLength(1);
    expect(flow.getSnapshot().value).toBe("idle");
  });

  it("takes no second merge while it is reading or delivering", async () => {
    const reading = deferred<IdentityReferences>();
    const { flow, host } = await started(() => reading.promise);
    host.answers = [applied, applied];
    flow.send({ type: "merge", ...merge });
    flow.send({ type: "merge", ...merge, merged: ["who_other"] });
    reading.resolve(referencing());
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(host.sent[0]?.mutations).toMatchObject([{ merged: ["who_old"] }]);
  });

  it("sends nothing when the references cannot be read, and keeps why until the next start", async () => {
    let fails = true;
    const { flow, host } = await started(() => (fails ? Promise.reject(new Error("the list is down")) : Promise.resolve(referencing())));
    flow.send({ type: "merge", ...merge });
    await settled();
    expect(host.sent).toEqual([]);
    expect(flow.getSnapshot().context.failure).toBe("the list is down");
    expect(flow.getSnapshot().value).toBe("idle");
    fails = false;
    host.answers = [applied];
    flow.send({ type: "merge", ...merge });
    await settled();
    expect(flow.getSnapshot().context.failure).toBeUndefined();
    expect(host.sent).toHaveLength(1);
  });

});

describe("a merge that does not land", () => {
  it("is not told of a rejection, which the manager keeps under the merge's edit key", async () => {
    const { flow, host, merged, writes } = await started(() => Promise.resolve(referencing()));
    host.answers = [rejected];
    flow.send({ type: "merge", ...merge });
    await settled();
    expect(merged).toEqual([]);
    expect(Object.values(writes()).map((entry) => entry.snapshot.context.about.edit)).toEqual(["merge-identities"]);
    expect(flow.getSnapshot().value).toBe("idle");
  });

  it("leaves the write to land when the flow is stopped while it waits", async () => {
    const held = deferred<Answered<HttpFailure>>();
    const { flow, host, saves, merged } = await started(() => Promise.resolve(referencing()));
    host.answers = [held.promise];
    flow.send({ type: "merge", ...merge });
    await settled();
    flow.stop();
    held.resolve(applied);
    await settled();
    expect(saves).toHaveLength(1);
    expect(merged).toEqual([]);
  });
});
