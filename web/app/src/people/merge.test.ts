// E6: a merge reads every journey referring to either identity, then delivers one deployment
// patch naming each at the revision read; the write is the edit manager's, so stopping the flow
// never stops it.
import type { Answered, HttpFailure } from "@cairn/client";
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { startRoot } from "../data/edits.test-support.ts";
import { deferred, settled } from "../data/fake.test-support.ts";
import type { JourneySummary } from "../data/reads.ts";
import { mergeFlow, type Merge, type Merged } from "./merge.ts";

const merge: Merge = { survivor: "who_keep", merged: ["who_old"], deployment: 7 };
const summary = (id: string, revision: number) => ({ id, revision }) as JourneySummary;
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt: { patch_id: "p_x", domain: "deployment", content_hash: "h", revision: 8 }, consequences: {} } };
const rejected: Answered<HttpFailure> = { outcome: "rejected", rejection: { rejection: "invalid", violations: [] } };

async function started(referencing: (identities: readonly string[]) => Promise<JourneySummary[]>) {
  const root = await startRoot();
  const flow = createActor(mergeFlow, { input: { edits: root.edits, key: "merge-identities", referencing } });
  const merged: Merged[] = [];
  flow.on("merged", (each) => merged.push(each));
  flow.start();
  return { ...root, flow, merged };
}

describe("a merge", () => {
  it("names every journey read, at the revision read, in a deployment patch against the deployment revision asked", async () => {
    const read: string[][] = [];
    const { flow, host, merged } = await started((identities) => {
      read.push([...identities]);
      return Promise.resolve([summary("j_one", 4), summary("j_two", 9)]);
    });
    host.answers = [applied];
    flow.send({ type: "merge", ...merge });
    await settled();
    expect(read).toEqual([["who_keep", "who_old"]]);
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
    const reading = deferred<JourneySummary[]>();
    const { flow, host } = await started(() => reading.promise);
    host.answers = [applied, applied];
    flow.send({ type: "merge", ...merge });
    flow.send({ type: "merge", ...merge, merged: ["who_other"] });
    reading.resolve([]);
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(host.sent[0]?.mutations).toMatchObject([{ merged: ["who_old"] }]);
  });

  it("sends nothing when the journeys cannot be read, and keeps why until the next start", async () => {
    let fails = true;
    const { flow, host } = await started(() => (fails ? Promise.reject(new Error("the list is down")) : Promise.resolve([])));
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
    const { flow, host, merged, writes } = await started(() => Promise.resolve([]));
    host.answers = [rejected];
    flow.send({ type: "merge", ...merge });
    await settled();
    expect(merged).toEqual([]);
    expect(Object.values(writes()).map((entry) => entry.snapshot.context.about.edit)).toEqual(["merge-identities"]);
    expect(flow.getSnapshot().value).toBe("idle");
  });

  it("leaves the write to land when the flow is stopped while it waits", async () => {
    const held = deferred<Answered<HttpFailure>>();
    const { flow, host, saves, merged } = await started(() => Promise.resolve([]));
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
