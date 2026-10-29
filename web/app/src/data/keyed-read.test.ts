// H6, A19: the keyed read's machine over a fake source whose reads the test answers by hand and a
// stream it drives by hand, on a simulated clock that knows which timers are set.
import { backoffDelayMs, type RevisionOf, type StreamEvent, type SubscriptionEvent, type Tick } from "@cairn/client";
import { HostFailure } from "@cairn/wasm";
import { createActor, type AnyActorRef } from "xstate";
import { describe, expect, it } from "vitest";

import { CountingClock, deferred } from "./fake.test-support.ts";
import { Missing, ReadFailed, Skewed } from "./host.ts";
import {
  DERIVE,
  PROJECTION,
  keyedRead,
  previewPolicy,
  readLag,
  readView,
  sameKey,
  valueOf,
  worstLag,
  type ReadKey,
  type ReadPolicy,
  type ReadSource,
  type Ticks,
  type WorkEvent,
} from "./keyed-read.ts";

/** What a fake read answers: a label, and the revisions it holds. */
interface Answer {
  label: string;
  holds?: { of: RevisionOf; revision: number }[];
}

const journey: RevisionOf = { domain: { journey: "j_one" } };
const other: RevisionOf = { domain: { journey: "j_other" } };
const at = (revision: number, of: RevisionOf = journey): Tick => ({ of, revision });

const answer = (label: string, ...holds: [RevisionOf, number][]): Answer => ({ label, holds: holds.map(([of, revision]) => ({ of, revision })) });

/** A stream that records who watches what and tells every watcher what the test sends. */
function fakeStream() {
  const watchers = new Map<string, { names: readonly string[]; to: AnyActorRef }>();
  const shown = new Map<string, AnyActorRef>();
  const working = new Set<string>();
  const events: string[] = [];
  return {
    watchers,
    shown,
    working,
    events,
    send(event: SubscriptionEvent | { type: "shown"; id: string; read: AnyActorRef } | { type: "hidden"; id: string } | WorkEvent) {
      events.push(event.type);
      if (event.type === "working") {
        working.add(event.id);
      } else if (event.type === "rested") {
        working.delete(event.id);
      } else if (event.type === "watch") {
        watchers.set(event.id, { names: event.names, to: event.to });
      } else if (event.type === "unwatch") {
        watchers.delete(event.id);
      } else if (event.type === "shown") {
        shown.set(event.id, event.read);
      } else if (event.type === "hidden") {
        shown.delete(event.id);
      }
    },
    tell(event: StreamEvent) {
      for (const { to } of [...watchers.values()]) {
        to.send(event);
      }
    },
  };
}

interface Options {
  policy?: ReadPolicy;
  ticks?: Partial<Ticks<Answer>>;
  release?: boolean;
}

/** A read of `Answer`s, keyed by a label; each read waits for the test to answer it. */
function harness(options: Options = {}) {
  const clock = new CountingClock();
  const stream = fakeStream();
  const state = { compatible: true, latched: 0, now: 0 };
  const calls: { args: string; held: Answer | undefined; signal: AbortSignal; compatible: () => boolean; resolve: (value: Answer) => void; reject: (reason: unknown) => void }[] = [];
  const released: Answer[] = [];
  const source: ReadSource<string, Answer> = {
    read: (args, readAt) => {
      const pending = deferred<Answer>();
      calls.push({ args, held: readAt.held, signal: readAt.signal, compatible: readAt.compatible, resolve: pending.resolve, reject: pending.reject });
      return pending.promise;
    },
    policy: options.policy ?? PROJECTION,
    ...(options.ticks === undefined
      ? {}
      : {
          ticks: (): Ticks<Answer> => ({
            watching: ["journey:j_one"],
            about: () => true,
            holds: (value) => value.holds ?? [],
            subject: journey,
            ...options.ticks,
          }),
        }),
    ...(options.release === true ? { release: (value: Answer) => released.push(value) } : {}),
  };
  const env = {
    stream,
    compatible: () => state.compatible,
    latch: () => {
      state.latched += 1;
      state.compatible = false;
    },
    now: () => state.now,
  };
  const ref = createActor(keyedRead, { clock, input: { source, env } }).start();
  const keyed = (of: string, freshness: string[] = [], args = of): ReadKey<string> => ({ of: [of], freshness, args });
  return {
    clock,
    stream,
    state,
    calls,
    released,
    ref,
    keyed,
    want: (key: ReadKey<string> | undefined) => {
      ref.send({ type: "want", key });
    },
    view: (of: string | undefined, freshness?: string[]) => readView<Answer>(ref.getSnapshot(), of === undefined ? undefined : [of], freshness),
    /** Lets the read's answer reach the machine. */
    flush: async () => {
      for (let turn = 0; turn < 6; turn += 1) {
        await Promise.resolve();
      }
    },
    tick: (tick: Tick) => {
      stream.tell({ type: "stream.tick", tick });
    },
    open: () => {
      stream.tell({ type: "stream.opened" });
    },
  };
}

const failed503 = () => new ReadFailed({ status: 503, message: "the host is down" });

describe("a key that changes while a read is out", () => {
  it("shows only the newest `of`, whichever read answers last", async () => {
    const h = harness();
    h.want(h.keyed("K1"));
    h.want(h.keyed("K2"));
    h.want(h.keyed("K3"));
    expect(h.calls.map((call) => call.args)).toEqual(["K1", "K2", "K3"]);
    expect(h.calls.map((call) => call.signal.aborted)).toEqual([true, true, false]);
    h.calls[2]?.resolve(answer("three"));
    await h.flush();
    expect(h.view("K3")).toEqual({ status: "current", value: answer("three") });
    h.calls[1]?.resolve(answer("two"));
    h.calls[0]?.resolve(answer("one"));
    await h.flush();
    expect(h.view("K3")).toEqual({ status: "current", value: answer("three") });
    expect(h.view("K1")).toEqual({ status: "loading" });
  });

  it("keeps the freshness it began with, reads the newest once, and never the one in between", async () => {
    const h = harness();
    h.want(h.keyed("K", ["F1"]));
    h.want(h.keyed("K", ["F2"]));
    h.want(h.keyed("K", ["F3"]));
    expect(h.calls.map((call) => call.args)).toEqual(["K"]);
    h.calls[0]?.resolve(answer("from one"));
    await h.flush();
    expect(h.calls).toHaveLength(2);
    // What landed is read from F1, so it is not current for F3.
    expect(h.view("K", ["F1"])).toEqual({ status: "current", value: answer("from one") });
    expect(h.view("K", ["F3"])).toEqual({ status: "refreshing", value: answer("from one") });
    h.calls[1]?.resolve(answer("from three"));
    await h.flush();
    expect(h.view("K", ["F3"])).toEqual({ status: "current", value: answer("from three") });
    expect(h.calls[1]?.held).toEqual(answer("from one"));
  });

  it("changes nothing for an equal key, even with arguments built afresh", async () => {
    const h = harness();
    h.want(h.keyed("K", ["F1"], "first args"));
    h.calls[0]?.resolve(answer("one"));
    await h.flush();
    const before = h.ref.getSnapshot();
    h.want(h.keyed("K", ["F1"], "second args"));
    expect(h.ref.getSnapshot()).toBe(before);
    expect(h.calls).toHaveLength(1);
  });
});

describe("a key taken back while its read is out", () => {
  it("is read when it is asked for again, though the read that was cancelled had begun for it", async () => {
    const h = harness();
    h.want(h.keyed("K"));
    h.want(undefined);
    expect(h.calls[0]?.signal.aborted).toBe(true);
    h.want(h.keyed("K"));
    expect(h.calls).toHaveLength(2);
    h.calls[1]?.resolve(answer("one"));
    await h.flush();
    expect(h.view("K")).toEqual({ status: "current", value: answer("one") });
  });

  it("is read for its new freshness when it is asked for again, keeping the older answer meanwhile", async () => {
    const h = harness();
    h.want(h.keyed("K", ["F1"]));
    h.calls[0]?.resolve(answer("from one"));
    await h.flush();
    h.want(h.keyed("K", ["F2"]));
    h.want(undefined);
    h.want(h.keyed("K", ["F2"]));
    expect(h.calls).toHaveLength(3);
    expect(h.view("K", ["F2"])).toEqual({ status: "refreshing", value: answer("from one") });
    h.calls[2]?.resolve(answer("from two"));
    await h.flush();
    expect(h.view("K", ["F2"])).toEqual({ status: "current", value: answer("from two") });
  });

  it("asks for nothing when it is asked for again with its answer in hand", async () => {
    const h = harness();
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("one"));
    await h.flush();
    h.want(undefined);
    h.want(h.keyed("K"));
    expect(h.calls).toHaveLength(1);
    expect(h.view("K")).toEqual({ status: "current", value: answer("one") });
  });
});

describe("a failed read", () => {
  it("keeps the data of a failed refresh, retries after the backoff, and waits longer after the next failure", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 1]));
    await h.flush();
    h.open();
    h.tick(at(2));
    expect(h.calls).toHaveLength(2);
    h.calls[1]?.reject(failed503());
    await h.flush();
    expect(h.view("K", [])).toMatchObject({ status: "behind", value: answer("v", [journey, 1]), retrying: true });
    h.clock.increment(backoffDelayMs(0) - 1);
    expect(h.calls).toHaveLength(2);
    h.clock.increment(1);
    expect(h.calls).toHaveLength(3);
    h.calls[2]?.reject(failed503());
    await h.flush();
    h.clock.increment(backoffDelayMs(0));
    expect(h.calls).toHaveLength(3);
    h.clock.increment(backoffDelayMs(1) - backoffDelayMs(0));
    expect(h.calls).toHaveLength(4);
    h.calls[3]?.resolve(answer("v2", [journey, 2]));
    await h.flush();
    expect(h.view("K", [])).toEqual({ status: "current", value: answer("v2", [journey, 2]) });
  });

  it("shows a first failure as failed, not behind, and tries again", async () => {
    const h = harness();
    h.want(h.keyed("K"));
    h.calls[0]?.reject(failed503());
    await h.flush();
    expect(h.view("K", [])).toMatchObject({ status: "failed", retrying: true });
    h.clock.increment(backoffDelayMs(0));
    h.calls[1]?.resolve(answer("v"));
    await h.flush();
    expect(h.view("K", [])).toMatchObject({ status: "current" });
  });

  it.each([
    ["an authentication failure", new ReadFailed({ status: 401, message: "sign in" }), "failed"],
    ["a refusal from the engine", new HostFailure({ error: "aborted", message: "the module stopped" }), "failed"],
  ])("does not retry %s", async (_, thrown, status) => {
    const h = harness();
    h.want(h.keyed("K"));
    h.calls[0]?.reject(thrown);
    await h.flush();
    expect(h.view("K", [])).toMatchObject({ status, retrying: false });
    expect(h.clock.pending).toEqual([]);
    h.clock.increment(60_000);
    expect(h.calls).toHaveLength(1);
  });

});

describe("a read that ends", () => {
  it("shows a missing subject as gone and does not retry", async () => {
    const h = harness();
    h.want(h.keyed("K"));
    h.calls[0]?.reject(new Missing("j_one"));
    await h.flush();
    expect(h.view("K")).toEqual({ status: "gone" });
    h.clock.increment(60_000);
    expect(h.calls).toHaveLength(1);
  });

  it("latches version skew, and starts nothing more", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.reject(new Skewed());
    await h.flush();
    expect(h.state.latched).toBe(1);
    expect(h.view("K", [])).toMatchObject({ status: "failed", error: { kind: "skew" }, retrying: false });
    h.open();
    h.tick(at(5));
    h.clock.increment(60_000);
    expect(h.calls).toHaveLength(1);
  });

  it("shows an error only for the freshness it failed at", async () => {
    const h = harness();
    h.want(h.keyed("K", ["F1"]));
    h.calls[0]?.reject(failed503());
    await h.flush();
    expect(h.view("K", ["F1"])).toMatchObject({ status: "failed" });
    h.want(h.keyed("K", ["F2"]));
    expect(h.view("K", ["F2"])).toEqual({ status: "loading" });
    expect(h.calls).toHaveLength(2);
    h.calls[1]?.resolve(answer("two"));
    await h.flush();
    h.want(h.keyed("K", ["F3"]));
    h.calls[2]?.reject(failed503());
    await h.flush();
    h.want(h.keyed("K", ["F4"]));
    expect(h.view("K", ["F4"])).toEqual({ status: "refreshing", value: answer("two") });
  });
});

describe("a retry that is no longer owed", () => {
  it("is gone after a read succeeds", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 1]));
    await h.flush();
    h.open();
    h.tick(at(2));
    h.calls[1]?.reject(failed503());
    await h.flush();
    expect(h.clock.pending).toEqual([backoffDelayMs(0)]);
    // The chip's Retry reopens the stream, whose current revisions ask again at once.
    h.open();
    h.tick(at(2));
    expect(h.calls).toHaveLength(3);
    h.calls[2]?.resolve(answer("v2", [journey, 2]));
    await h.flush();
    expect(h.clock.pending).toEqual([]);
    h.clock.increment(60_000);
    expect(h.calls).toHaveLength(3);
  });

  it("is gone once the subject no longer exists", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 1]));
    await h.flush();
    h.open();
    h.tick(at(2));
    h.calls[1]?.reject(failed503());
    await h.flush();
    h.tick(at(0));
    expect(h.view("K")).toEqual({ status: "gone" });
    expect(h.clock.pending).toEqual([]);
    h.clock.increment(60_000);
    expect(h.calls).toHaveLength(2);
  });

  it("aborts the read in flight when the subject no longer exists", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 1]));
    await h.flush();
    h.open();
    h.tick(at(2));
    h.tick(at(0));
    expect(h.calls[1]?.signal.aborted).toBe(true);
    h.calls[1]?.resolve(answer("late", [journey, 2]));
    await h.flush();
    expect(h.view("K")).toEqual({ status: "gone" });
  });
});

describe("ticks", () => {
  it("coalesce into one more read, and a tick the answer satisfies asks for none", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 1]));
    await h.flush();
    h.open();
    h.tick(at(2));
    expect(h.calls).toHaveLength(2);
    h.tick(at(3));
    h.tick(at(4));
    h.tick(at(5));
    expect(h.calls).toHaveLength(2);
    h.calls[1]?.resolve(answer("v2", [journey, 2]));
    await h.flush();
    expect(h.calls).toHaveLength(3);
    h.calls[2]?.resolve(answer("v5", [journey, 5]));
    await h.flush();
    expect(h.calls).toHaveLength(3);
    h.tick(at(5));
    h.tick(at(4));
    expect(h.calls).toHaveLength(3);
    // A tick during a read that the answer already holds causes none.
    h.tick(at(6));
    h.tick(at(7));
    h.calls[3]?.resolve(answer("v7", [journey, 7]));
    await h.flush();
    expect(h.calls).toHaveLength(4);
  });

  it("credits a tick for a domain the answer does not list, so it asks once per revision", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("index", [journey, 1]));
    await h.flush();
    h.open();
    h.tick(at(1, other));
    h.calls[1]?.resolve(answer("index", [journey, 1]));
    await h.flush();
    h.tick(at(1, other));
    expect(h.calls).toHaveLength(2);
    h.tick(at(2, other));
    expect(h.calls).toHaveLength(3);
  });

});

describe("ticks that fall short", () => {
  it("do not credit a revision the answer did not reach: the lag stays and the read is tried again with no further tick", async () => {
    const h = harness({ ticks: { reportsLag: true } });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 4]));
    await h.flush();
    h.open();
    h.state.now = 10;
    h.tick(at(5));
    h.calls[1]?.resolve(answer("still 4", [journey, 4]));
    await h.flush();
    expect(readLag(h.ref.getSnapshot())).toMatchObject({ shown: 4, announced: 5, since: 10, failed: false });
    expect(h.calls).toHaveLength(2);
    h.clock.increment(backoffDelayMs(0));
    expect(h.calls).toHaveLength(3);
    h.calls[2]?.resolve(answer("v5", [journey, 5]));
    await h.flush();
    expect(readLag(h.ref.getSnapshot())).toBeUndefined();
    expect(h.clock.pending).toEqual([]);
  });

  it("follow a deletion of a held domain during a read whose answer still lists it", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("index", [journey, 1], [other, 3]));
    await h.flush();
    h.open();
    h.tick(at(2));
    h.tick(at(0, other));
    h.calls[1]?.resolve(answer("index", [journey, 2], [other, 3]));
    await h.flush();
    expect(h.calls).toHaveLength(3);
  });

});

describe("a deletion while a read is out", () => {
  it("reads again when the domain deleted is one the read may still list, though it holds nothing of it yet", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.tick(at(0, other));
    // The read in flight began before the deletion: it may list the deleted domain.
    h.calls[0]?.resolve(answer("lists it"));
    await h.flush();
    expect(h.calls).toHaveLength(2);
    h.calls[1]?.resolve(answer("without it"));
    await h.flush();
    expect(h.view("K")).toEqual({ status: "current", value: answer("without it") });
  });

  it("asks for nothing when the domain deleted is one the read neither holds nor could list", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("one"));
    await h.flush();
    h.tick(at(0, other));
    await h.flush();
    expect(h.calls).toHaveLength(1);
  });
});

describe("ticks and the stream", () => {
  it("read again when the stream opens again, only for a read that asks to and holds a value", async () => {
    const reads = harness({ ticks: { rereadOnReopen: true } });
    reads.want(reads.keyed("K"));
    reads.open();
    expect(reads.calls).toHaveLength(1);
    reads.calls[0]?.resolve(answer("v"));
    await reads.flush();
    reads.open();
    expect(reads.calls).toHaveLength(2);
    const plain = harness({ ticks: {} });
    plain.want(plain.keyed("K"));
    plain.calls[0]?.resolve(answer("v"));
    await plain.flush();
    plain.open();
    expect(plain.calls).toHaveLength(1);
  });

  it("are listened to only while the read is keyed, under a new registration for a new `of`", () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K1"));
    const first = [...h.stream.watchers.keys()];
    expect(first).toHaveLength(1);
    h.want(h.keyed("K2"));
    const second = [...h.stream.watchers.keys()];
    expect(second).toHaveLength(1);
    expect(second).not.toEqual(first);
    h.ref.stop();
    expect(h.stream.watchers.size).toBe(0);
  });

  it("are not listened to while the read is parked, and are again once shown", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 1]));
    await h.flush();
    h.ref.send({ type: "park" });
    expect(h.stream.watchers.size).toBe(0);
    h.ref.send({ type: "show" });
    h.ref.send({ type: "show" });
    expect(h.stream.watchers.size).toBe(1);
  });
});

describe("a settle policy", () => {
  it("reads once, with the last key, after the key has held still", () => {
    const h = harness({ policy: previewPolicy(250) });
    h.want(h.keyed("A"));
    h.clock.increment(100);
    h.want(h.keyed("B"));
    h.clock.increment(100);
    h.want(h.keyed("C"));
    h.clock.increment(249);
    expect(h.calls).toHaveLength(0);
    expect(h.view("C", [])).toEqual({ status: "loading" });
    h.clock.increment(1);
    expect(h.calls.map((call) => call.args)).toEqual(["C"]);
  });

  it("reads at once, with the last key, when the person has finished typing", () => {
    const h = harness({ policy: previewPolicy(250) });
    h.want(h.keyed("A"));
    h.clock.increment(100);
    h.want(h.keyed("B"));
    h.ref.send({ type: "flush" });
    expect(h.calls.map((call) => call.args)).toEqual(["B"]);
    h.clock.increment(1000);
    expect(h.calls).toHaveLength(1);
  });
});

describe("the work it reports", () => {
  it("is under way while the read settles and reads, and over while it waits out a failure and once it rests", async () => {
    const h = harness({ policy: previewPolicy(250) });
    h.want(h.keyed("A"));
    expect(h.stream.working.size).toBe(1);
    h.clock.increment(250);
    expect(h.calls).toHaveLength(1);
    expect(h.stream.working.size).toBe(1);
    h.calls[0]?.reject(failed503());
    await h.flush();
    expect(h.view("A", [])).toMatchObject({ status: "failed", retrying: true });
    expect(h.stream.working.size).toBe(0);
    h.clock.increment(backoffDelayMs(0));
    expect(h.stream.working.size).toBe(1);
    h.calls[1]?.resolve(answer("a"));
    await h.flush();
    expect(h.view("A", [])).toEqual({ status: "current", value: answer("a") });
    expect(h.stream.working.size).toBe(0);
  });

  it("is under way again for a tick that asks for a newer revision, and ends with the read stopped mid-way", async () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("v", [journey, 1]));
    await h.flush();
    expect(h.stream.working.size).toBe(0);
    h.tick(at(2));
    expect(h.stream.working.size).toBe(1);
    h.ref.stop();
    expect(h.stream.working.size).toBe(0);
  });
});

describe("version skew", () => {
  it("stops a read that was settling, and starts nothing", () => {
    const h = harness({ policy: previewPolicy(250) });
    h.want(h.keyed("A"));
    h.clock.increment(100);
    h.state.compatible = false;
    h.clock.increment(150);
    expect(h.calls).toHaveLength(0);
    expect(h.view("A", [])).toMatchObject({ status: "failed", error: { kind: "skew" } });
  });

  it("does not retry a failure that was waiting", async () => {
    const h = harness({ policy: PROJECTION });
    h.want(h.keyed("A"));
    h.calls[0]?.reject(failed503());
    await h.flush();
    h.state.compatible = false;
    h.clock.increment(60_000);
    expect(h.calls).toHaveLength(1);
  });

  it("stops what derives but still answers a projection of what is held", async () => {
    const derives = harness({ policy: DERIVE });
    const projects = harness({ policy: PROJECTION });
    for (const each of [derives, projects]) {
      each.state.compatible = false;
      each.want(each.keyed("A"));
    }
    expect(derives.calls).toHaveLength(0);
    expect(projects.calls).toHaveLength(1);
    projects.calls[0]?.resolve(answer("projected"));
    await projects.flush();
    expect(valueOf(projects.view("A", []))).toEqual(answer("projected"));
  });

  it("lets the source ask whether the tab is still compatible before it starts work", () => {
    const h = harness({ policy: DERIVE });
    h.want(h.keyed("A"));
    expect(h.calls[0]?.compatible()).toBe(true);
    h.state.compatible = false;
    expect(h.calls[0]?.compatible()).toBe(false);
  });
});

describe("a value that owns a resource", () => {
  it("is released when replaced, when its subject is gone, when its `of` changes, and when stopped, once each", async () => {
    const h = harness({ ticks: {}, release: true });
    h.want(h.keyed("K"));
    h.calls[0]?.resolve(answer("one", [journey, 1]));
    await h.flush();
    expect(h.released).toEqual([]);
    h.open();
    h.tick(at(2));
    h.calls[1]?.resolve(answer("two", [journey, 2]));
    await h.flush();
    expect(h.released.map((value) => value.label)).toEqual(["one"]);
    h.tick(at(0));
    expect(h.released.map((value) => value.label)).toEqual(["one", "two"]);
    h.want(h.keyed("L"));
    h.calls[2]?.resolve(answer("three", [journey, 1]));
    await h.flush();
    h.want(h.keyed("M"));
    expect(h.released.map((value) => value.label)).toEqual(["one", "two", "three"]);
    h.calls[3]?.resolve(answer("four", [journey, 1]));
    await h.flush();
    h.ref.stop();
    expect(h.released.map((value) => value.label)).toEqual(["one", "two", "three", "four"]);
  });

  it("keeps its owner when a read answers the value it already holds", async () => {
    const h = harness({ ticks: {}, release: true });
    h.want(h.keyed("K"));
    const value = answer("one", [journey, 1]);
    h.calls[0]?.resolve(value);
    await h.flush();
    h.open();
    h.tick(at(2));
    h.calls[1]?.resolve(h.calls[1].held ?? value);
    await h.flush();
    expect(h.released).toEqual([]);
    h.ref.stop();
    expect(h.released).toEqual([value]);
  });
});

describe("a read the sync chip counts", () => {
  it("registers while keyed, under a new registration for a new `of`, and leaves when stopped", () => {
    const h = harness({ ticks: { reportsLag: true } });
    h.want(h.keyed("K1"));
    const first = [...h.stream.shown.keys()];
    expect(first).toHaveLength(1);
    expect([...h.stream.shown.values()]).toEqual([h.ref]);
    h.want(h.keyed("K2"));
    expect([...h.stream.shown.keys()]).toHaveLength(1);
    expect([...h.stream.shown.keys()]).not.toEqual(first);
    h.ref.stop();
    expect(h.stream.shown.size).toBe(0);
  });

  it("registers nothing when its ticks do not report lag", () => {
    const h = harness({ ticks: {} });
    h.want(h.keyed("K"));
    expect(h.stream.shown.size).toBe(0);
  });

  it("puts a failed refetch before a longer wait", () => {
    const wait = { subject: "journey:j_a", shown: 1, announced: 2, since: 1, failed: false };
    const failure = { subject: "journey:j_b", shown: 1, announced: 2, since: 9, failed: true };
    expect(worstLag([wait, failure, undefined])).toBe(failure);
    expect(worstLag([undefined, { ...wait, since: 5 }, wait])).toBe(wait);
    expect(worstLag([undefined])).toBeUndefined();
  });
});

describe("what a read shows", () => {
  it("shows nothing of another `of`, and nothing for no key", async () => {
    const h = harness();
    h.want(h.keyed("K1"));
    h.calls[0]?.resolve(answer("one"));
    await h.flush();
    expect(h.view("K1", [])).toMatchObject({ status: "current" });
    expect(h.view("K2", [])).toEqual({ status: "loading" });
    expect(h.view(undefined)).toEqual({ status: "idle" });
  });

  it("keeps the value held when nothing is wanted for now", async () => {
    const h = harness();
    h.want(h.keyed("K1"));
    h.calls[0]?.resolve(answer("one"));
    await h.flush();
    h.want(undefined);
    expect(h.view("K1")).toEqual({ status: "current", value: answer("one") });
    h.want(h.keyed("K1"));
    expect(h.calls).toHaveLength(1);
  });

  it("compares keys element by element", () => {
    expect(sameKey(["a", 1], ["a", 1])).toBe(true);
    expect(sameKey(["a", 1], ["a", 2])).toBe(false);
    expect(sameKey(["a"], ["a", undefined])).toBe(false);
    expect(sameKey(undefined, ["a"])).toBe(false);
  });
});
