// What each canvas read is keyed by: an answer is shown only for what it answered (another level
// is another graph), an earlier revision's answer stays until the next arrives, and an error
// never outlives the key it came from. The keys are what the hooks send; the machine they feed
// is tested in data/keyed-read.test.ts, here over the same view function the hooks use.
import { createActor } from "xstate";
import { describe, expect, test } from "vitest";

import { rowsKey } from "../acting/useRows.ts";
import type { Deriver } from "../data/host.ts";
import { PROJECTION, keyedRead, readView, type ReadEnv, type ReadKey, type ReadSource } from "../data/keyed-read.ts";
import type { Ready } from "../detail/model.ts";
import { joinedTraceKey, laidOutKey, projectedKey } from "./hooks.ts";
import type { Layouts } from "./layouts.ts";
import type { CanvasModel } from "./model.ts";

const deriver = {} as Deriver;
const layouts = {} as Layouts;
const ready = (revision: number): Ready => ({ key: { journey: "j_one", revision, deployment_revision: 1, today: "2026-10-06" } }) as unknown as Ready;
const model = (): CanvasModel => ({ cards: [], lines: [] });

/** A read that answers or fails as the test says, for whatever key it is given. */
function reading() {
  const env: ReadEnv = { stream: { send: () => undefined }, compatible: () => true, latch: () => undefined, now: () => 0 };
  const outcome: { fail: boolean } = { fail: false };
  const source: ReadSource<unknown, string> = {
    policy: PROJECTION,
    read: () => (outcome.fail ? Promise.reject(new Error("no node")) : Promise.resolve("answer")),
  };
  const actor = createActor(keyedRead, { input: { source, env } }).start();
  const settle = async () => {
    for (let turn = 0; turn < 6; turn += 1) {
      await Promise.resolve();
    }
  };
  return {
    outcome,
    ask: async (key: ReadKey<unknown> | undefined) => {
      actor.send({ type: "want", key });
      await settle();
    },
    show: (key: ReadKey<unknown> | undefined) => readView<string>(actor.getSnapshot(), key?.identity, key?.freshness),
  };
}

const level = (shown: string) => ({ projection: "level" as const, shown: [shown] as never });

describe("what a projection shows", () => {
  test("its own answer, kept through a newer revision until the next arrives", async () => {
    const read = reading();
    const first = projectedKey(deriver, ready(1), level("group"));
    await read.ask(first);
    expect(read.show(first)).toMatchObject({ status: "current", value: "answer" });
    const next = projectedKey(deriver, ready(2), level("group"));
    expect(next?.identity).toEqual(first?.identity);
    expect(read.show(next)).toMatchObject({ status: "refreshing", value: "answer" });
  });

  test("nothing of another request or another journey", async () => {
    const read = reading();
    await read.ask(projectedKey(deriver, ready(1), level("group")));
    const other = projectedKey(deriver, ready(1), level("action"));
    expect(read.show(other)).toEqual({ status: "loading" });
    expect(projectedKey(deriver, ready(1), level("group"))?.identity).not.toEqual(projectedKey(deriver, { key: { ...ready(1).key, journey: "j_two" } } as unknown as Ready, level("group"))?.identity);
    expect(read.show(projectedKey(deriver, ready(1), undefined))).toEqual({ status: "idle" });
  });

  test("an error only for the request and derivation it came from", async () => {
    const read = reading();
    read.outcome.fail = true;
    const failing = projectedKey(deriver, ready(1), level("group"));
    await read.ask(failing);
    expect(read.show(failing)).toMatchObject({ status: "failed", error: { message: "no node" } });
    // A new revision of the same request, and another request, show the load, not the old error.
    read.outcome.fail = false;
    const newer = projectedKey(deriver, ready(2), level("group"));
    expect(read.show(newer)).toEqual({ status: "loading" });
    expect(read.show(projectedKey(deriver, ready(1), level("action")))).toEqual({ status: "loading" });
  });
});

describe("what a layout shows", () => {
  test("a new model keeps the canvas laid out until its layout arrives, and shows no error of the old one", async () => {
    const read = reading();
    const first = laidOutKey(layouts, "j_one", "level", model());
    await read.ask(first);
    expect(read.show(first)).toMatchObject({ status: "current" });
    read.outcome.fail = true;
    const second = laidOutKey(layouts, "j_one", "level", model());
    expect(read.show(second)).toMatchObject({ status: "refreshing" });
    await read.ask(second);
    expect(read.show(second)).toMatchObject({ status: "behind", error: { message: "no node" } });
    read.outcome.fail = false;
    const third = laidOutKey(layouts, "j_one", "level", model());
    expect(read.show(third)).toMatchObject({ status: "refreshing" });
  });

  test("another domain's positions never hint this one, and no model is no read", async () => {
    const read = reading();
    await read.ask(laidOutKey(layouts, "j_one", "level", model()));
    expect(read.show(laidOutKey(layouts, "j_two", "level", model()))).toEqual({ status: "loading" });
    expect(laidOutKey(layouts, "j_one", "level", undefined)).toBeUndefined();
  });
});

describe("what the other canvas reads are keyed by", () => {
  test("a joined trace by the keys it joins, and the list's rows by the query", () => {
    expect(joinedTraceKey(deriver, ready(1), [])).toBeUndefined();
    const keys = joinedTraceKey(deriver, ready(1), ["n_a", "n_b"]);
    expect(keys?.identity).not.toEqual(joinedTraceKey(deriver, ready(1), ["n_a", "n_c"])?.identity);
    expect(keys?.freshness).not.toEqual(joinedTraceKey(deriver, ready(2), ["n_a", "n_b"])?.freshness);
    expect(rowsKey(deriver, ready(1), { status: ["open"] } as never).identity).not.toEqual(rowsKey(deriver, ready(1), { status: ["done"] } as never).identity);
  });
});
