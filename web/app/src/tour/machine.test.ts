// The tour's lifecycle with controlled promises: who owns the practice journey at each moment,
// what is written first, and what recovers when something fails. Each test asserts the
// ownership and recovery state, not the order of calls alone.
import { createActor, SimulatedClock } from "xstate";
import { describe, expect, it } from "vitest";

import type { Tour, TourWrite, TourWriteRequest } from "../data/host.ts";
import { expecting, PENDING_MS, tourMachine, type TourPorts, type TourSnapshot } from "./machine.ts";

const TODAY = "2026-10-06";

/** A promise the test settles. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

type Pending<T> = ReturnType<typeof deferred<T>>;

/** The world the tour acts on: a record that compare-and-sets as the host's does, and journeys that exist or not. */
function world(first: Tour = { state: "none", today: TODAY }) {
  const calls: string[] = [];
  const journeys = new Set<string>();
  let record = first;
  let counter = 0;
  const holds: { read?: Pending<Tour>; write?: Pending<TourWrite>; create?: Pending<boolean>; remove?: Pending<"gone" | "deleted" | "failed"> } = {};
  const ports: TourPorts = {
    read: () => {
      calls.push("read");
      return holds.read === undefined ? Promise.resolve(record) : holds.read.promise;
    },
    write: (request: TourWriteRequest) => {
      calls.push(`write ${request.state}`);
      const same = expecting(record).state === request.expect.state && (expecting(record).practice_journey ?? null) === (request.expect.practice_journey ?? null);
      const result: TourWrite = same ? { ok: true } : { ok: false, current: record };
      // A write the test holds is settled by the test: it changes nothing here.
      if (same && holds.write === undefined) {
        record = { today: TODAY, state: request.state, ...(request.practice_journey == null ? {} : { practice_journey: request.practice_journey }) };
      }
      return holds.write === undefined ? Promise.resolve(result) : holds.write.promise;
    },
    newJourneyId: () => `j_practice_${String((counter += 1))}`,
    create: (id) => {
      calls.push(`create ${id}`);
      journeys.add(id);
      return holds.create === undefined ? Promise.resolve(true) : holds.create.promise;
    },
    remove: (id) => {
      calls.push(`remove ${id}`);
      const there = journeys.delete(id);
      return holds.remove === undefined ? Promise.resolve(there ? "deleted" : "gone") : holds.remove.promise;
    },
  };
  return { ports, calls, journeys, holds, record: () => record, set: (next: Tour) => (record = next) };
}

/** Lets resolved promises and their transitions run. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

async function started(first?: Tour) {
  const w = world(first);
  const clock = new SimulatedClock();
  const actor = createActor(tourMachine, { input: { ports: w.ports }, clock }).start();
  await settle();
  const at = (): TourSnapshot => actor.getSnapshot();
  /** Lets the wait for a missing journey run out. */
  const wait = async () => {
    clock.increment(PENDING_MS);
    await settle();
  };
  return { w, actor, at, wait, send: actor.send.bind(actor), value: () => JSON.stringify(at().value) };
}

/** Starts the tour and confirms the journey it made is there. */
async function running() {
  const t = await started();
  t.send({ type: "start" });
  await settle();
  const run = t.at().context.run ?? "";
  t.send({ type: "journey", id: run, exists: true });
  return { ...t, run };
}

describe("starting", () => {
  it("writes the record naming the journey before it creates the journey", async () => {
    const t = await started();
    expect(t.value()).toBe('"idle"');
    t.send({ type: "start" });
    await settle();
    expect(t.w.calls.slice(1)).toEqual(["write started", "create j_practice_1"]);
    expect(t.w.record()).toMatchObject({ state: "started", practice_journey: "j_practice_1" });
    expect(t.value()).toBe('{"running":"confirming"}');
  });

  it("does not create anything when the record could not be written, and offers to try again", async () => {
    const t = await started();
    t.w.holds.write = deferred();
    t.send({ type: "start" });
    await settle();
    t.w.holds.write.resolve({ ok: false, failure: { status: 0, message: "offline" } });
    await settle();
    expect(t.w.calls).not.toContain("create j_practice_1");
    expect(t.at().context.failure).toBe("start");
    expect(t.value()).toBe('"failed"');
  });

  it("does not call a run that never got a journey resumable when the failure is closed", async () => {
    const t = await started();
    t.w.holds.create = deferred();
    t.send({ type: "start" });
    await settle();
    t.w.holds.create.resolve(false);
    await settle();
    t.send({ type: "close" });
    expect(t.value()).toBe('"idle"');
    expect(t.at().context.run).toBeUndefined();
  });

  it("leaves a started record naming a journey that is not there when the create fails, and retries over that record with a new journey", async () => {
    const t = await started();
    t.w.holds.create = deferred();
    t.send({ type: "start" });
    await settle();
    t.w.holds.create.resolve(false);
    await settle();
    expect(t.at().context.failure).toBe("start");
    expect(t.w.record()).toMatchObject({ state: "started", practice_journey: "j_practice_1" });
    delete t.w.holds.create;
    t.send({ type: "retry" });
    await settle();
    // The retry expected the stale started record, so it replaced it.
    expect(t.w.record()).toMatchObject({ state: "started", practice_journey: "j_practice_2" });
    expect(t.at().context.failure).toBeUndefined();
    expect(t.value()).toBe('{"running":"confirming"}');
  });

  it("ends up on one run when another tab started first", async () => {
    const t = await started();
    // Another tab starts after this one read the record.
    t.w.set({ state: "started", practice_journey: "j_other", today: TODAY });
    t.send({ type: "start" });
    await settle();
    expect(t.w.calls).not.toContain("create j_practice_1");
    expect(t.at().context.tour?.practice_journey).toBe("j_other");
    expect(t.value()).toBe('{"checking":"asking"}');
    t.send({ type: "journey", id: "j_other", exists: true });
    expect(t.value()).toBe('{"running":"live"}');
    expect(t.at().context.run).toBe("j_other");
  });
});

describe("a record read at load", () => {
  it("treats a started record whose journey is confirmed missing as no run, and starts over it", async () => {
    const t = await started({ state: "started", practice_journey: "j_gone", today: TODAY });
    expect(t.value()).toBe('{"checking":"asking"}');
    t.send({ type: "journey", id: "j_gone", exists: false });
    expect(t.value()).toBe('{"checking":"pending"}');
    await t.wait();
    expect(t.value()).toBe('"idle"');
    t.send({ type: "start" });
    await settle();
    expect(t.w.record()).toMatchObject({ state: "started", practice_journey: "j_practice_1" });
  });

  it("continues a started record whose journey is there", async () => {
    const t = await started({ state: "started", practice_journey: "j_there", today: TODAY });
    t.send({ type: "journey", id: "j_there", exists: true });
    expect(t.value()).toBe('{"running":"live"}');
  });

  it("ignores what a read says of another journey", async () => {
    const t = await started({ state: "started", practice_journey: "j_there", today: TODAY });
    t.send({ type: "journey", id: "j_elsewhere", exists: false });
    expect(t.value()).toBe('{"checking":"asking"}');
  });

  it("offers nothing when the read failed, and reads again on request", async () => {
    const w = world();
    w.holds.read = deferred();
    const actor = createActor(tourMachine, { input: { ports: w.ports } }).start();
    w.holds.read.reject(new Error("offline"));
    await settle();
    expect(actor.getSnapshot().value).toBe("unreadable");
    delete w.holds.read;
    actor.send({ type: "reload" });
    await settle();
    expect(actor.getSnapshot().value).toBe("idle");
  });
});

describe("a run in progress", () => {
  it("waits for a journey another tab is still creating, and goes on with it when it appears", async () => {
    const t = await started({ state: "started", practice_journey: "j_other", today: TODAY });
    t.send({ type: "journey", id: "j_other", exists: false });
    t.send({ type: "journey", id: "j_other", exists: true });
    expect(t.value()).toBe('{"running":"live"}');
  });

  it("adopts the ending another tab wrote instead of offering the tour again", async () => {
    const t = await running();
    t.w.set({ state: "finished", today: TODAY });
    t.send({ type: "journey", id: t.run, exists: false });
    await t.wait();
    expect(t.at().context.tour?.state).toBe("finished");
    expect(t.at().context.run).toBeUndefined();
  });

  it("ends quietly, writing nothing, when its journey disappears and stays gone", async () => {
    const t = await running();
    const writes = t.w.calls.filter((call) => call.startsWith("write")).length;
    t.send({ type: "journey", id: t.run, exists: false });
    await t.wait();
    expect(t.value()).toBe('"idle"');
    expect(t.w.calls.filter((call) => call.startsWith("write")).length).toBe(writes);
  });

  it("does not take the journey's first read, before it exists, for its disappearance", async () => {
    const t = await started();
    t.w.holds.create = deferred();
    t.send({ type: "start" });
    await settle();
    t.w.holds.create.resolve(true);
    await settle();
    t.send({ type: "journey", id: "j_practice_1", exists: false });
    expect(t.value()).toBe('{"running":"confirming"}');
  });
});

describe("ending", () => {
  it("leaves first, then deletes the journey, then records the ending", async () => {
    const t = await running();
    t.send({ type: "where", under: true });
    t.send({ type: "finish" });
    await settle();
    expect(t.value()).toBe('{"ending":"leaving"}');
    expect(t.w.calls.some((call) => call.startsWith("remove"))).toBe(false);
    t.send({ type: "where", under: false });
    await settle();
    expect(t.w.calls.slice(-2)).toEqual([`remove ${t.run}`, "write finished"]);
    expect(t.w.journeys.has(t.run)).toBe(false);
    expect(t.w.record().state).toBe("finished");
    // A finish shows the end card; a skip shows nothing.
    expect(t.value()).toBe('"ended"');
    t.send({ type: "close" });
    expect(t.value()).toBe('"idle"');
  });

  it("goes straight to deleting when the person is already elsewhere, and a skip closes with nothing to show", async () => {
    const t = await running();
    t.send({ type: "skip" });
    await settle();
    expect(t.w.record().state).toBe("dismissed");
    expect(t.value()).toBe('"idle"');
  });
});

describe("an ending that does not go through", () => {
  it("keeps the run recoverable when the journey cannot be deleted, and retries the deletion", async () => {
    const t = await running();
    t.w.holds.remove = deferred();
    t.send({ type: "skip" });
    await settle();
    t.w.holds.remove.resolve("failed");
    await settle();
    expect(t.at().context.failure).toBe("cleanup");
    // The record still says the run is on.
    expect(t.w.record()).toMatchObject({ state: "started", practice_journey: t.run });
    delete t.w.holds.remove;
    t.send({ type: "retry" });
    await settle();
    expect(t.w.record().state).toBe("dismissed");
    expect(t.at().context.failure).toBeUndefined();
  });

  it("a journey someone else already deleted counts as deleted", async () => {
    const t = await running();
    t.w.journeys.delete(t.run);
    t.send({ type: "skip" });
    await settle();
    expect(t.w.record().state).toBe("dismissed");
  });

});

describe("an ending whose record is not written", () => {
  it("still closes when the closing write fails, keeps the offer shut, and saves again on request", async () => {
    const t = await running();
    t.w.holds.write = deferred();
    t.send({ type: "finish" });
    await settle();
    t.w.holds.write.resolve({ ok: false, failure: { status: 0, message: "offline" } });
    await settle();
    expect(t.w.journeys.has(t.run)).toBe(false);
    expect(t.at().context.unsaved).toBe(true);
    // The end card shows once; the record is unchanged, still naming the deleted journey.
    expect(t.value()).toBe('"ended"');
    t.send({ type: "close" });
    expect(t.value()).toBe('"idle"');
    expect(t.at().context.unsaved).toBe(true);
    delete t.w.holds.write;
    t.w.set({ state: "started", practice_journey: t.run, today: TODAY });
    t.send({ type: "save" });
    await settle();
    expect(t.at().context.unsaved).toBe(false);
    expect(t.w.record().state).toBe("finished");
  });

  it("changes nothing when a newer run exists: an old run's ending is not written over it", async () => {
    const t = await running();
    // Another tab's newer run replaced the record while this one was ending.
    t.w.set({ state: "started", practice_journey: "j_newer", today: TODAY });
    t.send({ type: "skip" });
    await settle();
    expect(t.w.record()).toMatchObject({ state: "started", practice_journey: "j_newer" });
    expect(t.value()).toBe('{"checking":"asking"}');
    expect(t.at().context.tour?.practice_journey).toBe("j_newer");
  });

  it("ignores a journey disappearing while the ending is in flight", async () => {
    const t = await running();
    t.w.holds.remove = deferred();
    t.send({ type: "skip" });
    await settle();
    t.send({ type: "journey", id: t.run, exists: false });
    expect(t.value()).toBe('{"ending":"deleting"}');
    t.w.holds.remove.resolve("deleted");
    await settle();
    expect(t.w.record().state).toBe("dismissed");
  });
});

describe("Not now", () => {
  it("writes dismissed over the record as read", async () => {
    const t = await started();
    t.send({ type: "dismiss" });
    await settle();
    expect(t.w.record().state).toBe("dismissed");
    expect(t.value()).toBe('"idle"');
  });

  it("keeps the offer shut for this tab, with the write to retry, when it did not land", async () => {
    const t = await started();
    t.w.holds.write = deferred();
    t.send({ type: "dismiss" });
    await settle();
    t.w.holds.write.resolve({ ok: false, failure: { status: 0, message: "offline" } });
    await settle();
    expect(t.at().context).toMatchObject({ unsaved: true, ending: "dismissed" });
    expect(t.w.record().state).toBe("none");
  });
});
