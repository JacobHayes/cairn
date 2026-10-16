// H5: a write is delivered once however many times it is sent, and resent with its patch id
// until it is answered. The write's states, its resends (the backoff, the wait for the
// connection, the limit, the restore), and what each answer makes of it (A18, A19).
import { RESUBMISSION_COUNT_MAX, backoffDelayMs, type Answered, type HttpFailure, type Patch, type PatchAnswer, type Rejection } from "@cairn/client";
import { SimulatedClock, createActor } from "xstate";
import { describe, expect, it } from "vitest";

import type { Delivery } from "./edit-record.ts";
import { FakeHost, deferred, settled, writeEnv } from "./fake.test-support.ts";
import { RESEND_COUNT_MAX, recordOf, writeMachine, writeView, type WriteView } from "./write-machine.ts";
import { patchOf } from "./writes.ts";

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const alreadyApplied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "already_applied", receipt } as PatchAnswer };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };
const busy: Answered<HttpFailure> = { outcome: "failed", error: { status: 503, message: "busy" } };
const unauthorized: Answered<HttpFailure> = { outcome: "failed", error: { status: 401, message: "sign in" } };
const stale = (current: number): Answered<HttpFailure> => ({
  outcome: "rejected",
  rejection: { rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current }], intervening: [] },
});
const stales = (count: number) => Array.from({ length: count }, (_, at) => stale(5 + at));

const patch: Patch = patchOf({ target: { journey: "j_one" }, baseRevision: 3, mutations: [{ op: "set_node_field", node: "n_a", value: { title: "Renamed" } }] }, "p_one");
const about = { edit: "title:j_one:n_a", label: "Draft the plan", address: "/journeys/j_one/n_a" };

/** A write over a fake host and a simulated clock, started with `restored` when given. */
function setup(restored?: { delivery: Delivery; resubmitted: number }, answers: Answered<HttpFailure>[] = []) {
  const host = new FakeHost();
  host.answers = answers;
  const { env, switches, saves } = writeEnv(host);
  const clock = new SimulatedClock();
  const actor = createActor(writeMachine, { input: { id: patch.id, command: { kind: "patch", patch }, about, env, restored }, clock });
  actor.start();
  const view = (): WriteView => writeView(actor.getSnapshot());
  /** Advances the clock, then lets the answers that came back run. */
  const advance = async (ms: number) => {
    clock.increment(ms);
    await settled();
  };
  return { host, switches, saves, actor, view, advance };
}

describe("a write's delivery", () => {
  it("sends once however many times it is submitted", async () => {
    const { host, actor, view } = setup();
    host.answers = [applied];
    actor.send({ type: "submit" });
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(view().state).toBe("landed");
  });

  it("refuses a second submit while the first is out, not only after it was answered", async () => {
    const { host, actor, view } = setup();
    const held = deferred<Answered<HttpFailure>>();
    host.answers = [held.promise];
    actor.send({ type: "submit" });
    await settled();
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(view().state).toBe("sending");
    held.resolve(applied);
    await settled();
    expect(view().state).toBe("landed");
  });

  it("retries a safe stale answer on the reported revision, lands once, and records one save", async () => {
    const { host, saves, actor, view } = setup();
    host.answers = [stale(5), applied];
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent.map((sent) => [sent.id, sent.base_revision])).toEqual([["p_one", 3], ["p_one", 5]]);
    expect(view()).toMatchObject({ state: "landed", resubmitted: 1 });
    expect(saves).toHaveLength(1);
    expect(saves[0]?.command.patch.base_revision).toBe(5);
  });

  it("resends a lost write with its id and content after the first wait, and settles on already_applied", async () => {
    const { host, saves, actor, view, advance } = setup();
    host.answers = [lost, alreadyApplied];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: true, failure: { status: 0 } });
    await advance(backoffDelayMs(0) - 1);
    expect(host.sent).toHaveLength(1);
    await advance(1);
    expect(host.sent).toHaveLength(2);
    expect(host.sent[1]).toEqual(host.sent[0]);
    expect(view().state).toBe("landed");
    expect(saves).toHaveLength(1);
  });

  it("resends the patch it last sent, rebased, not the original", async () => {
    const { host, actor, advance } = setup();
    host.answers = [stale(5), lost, applied];
    actor.send({ type: "submit" });
    await settled();
    await advance(backoffDelayMs(0));
    expect(host.sent.map((sent) => sent.base_revision)).toEqual([3, 5, 5]);
  });

});

describe("a write that got no answer", () => {
  it("backs off between resends, then stops after the limit and offers Retry by hand", async () => {
    const { host, actor, view, advance } = setup();
    host.answers = Array.from({ length: RESEND_COUNT_MAX + 3 }, () => lost);
    actor.send({ type: "submit" });
    await settled();
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      expect(view()).toMatchObject({ state: "unconfirmed", retrying: true });
      await advance(backoffDelayMs(resend) - 1);
      expect(host.sent).toHaveLength(1 + resend);
      await advance(1);
      expect(host.sent).toHaveLength(2 + resend);
    }
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: false });
    await advance(60_000);
    expect(host.sent).toHaveLength(1 + RESEND_COUNT_MAX);
    actor.send({ type: "retry" });
    await settled();
    expect(host.sent).toHaveLength(2 + RESEND_COUNT_MAX);
  });

  it("counts the safe retry across resends: a resend does not start the bound again", async () => {
    const { host, actor, view, advance } = setup();
    host.answers = [...stales(20), lost, ...stales(40)];
    actor.send({ type: "submit" });
    await settled();
    await advance(backoffDelayMs(0));
    // The original, its 20 rebased sends, the resend, then the rest of the 32 allowed in all.
    expect(host.sent).toHaveLength(1 + 20 + 1 + (RESUBMISSION_COUNT_MAX - 20));
    expect(view()).toMatchObject({ state: "rejected", resubmitted: RESUBMISSION_COUNT_MAX });
  });

  it("carries the bound across a reload", async () => {
    const { host, view } = setup({ delivery: { state: "unconfirmed" }, resubmitted: 20 }, stales(40));
    await settled();
    expect(host.sent).toHaveLength(1 + (RESUBMISSION_COUNT_MAX - 20));
    expect(view().state).toBe("rejected");
  });
});

describe("a write waiting for the connection", () => {
  it("sends nothing offline, spends no attempt, and resends when the connection is back", async () => {
    const { host, switches, actor, view, advance } = setup();
    switches.connected = false;
    host.answers = [lost, ...Array.from({ length: RESEND_COUNT_MAX }, () => lost)];
    actor.send({ type: "submit" });
    await settled();
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: true });
    switches.connected = true;
    actor.send({ type: "wake" });
    await settled();
    expect(host.sent).toHaveLength(2);
    // The whole budget is still there: five resends in all, after the minute spent offline.
    for (let resend = 1; resend < RESEND_COUNT_MAX; resend += 1) {
      await advance(backoffDelayMs(resend));
    }
    expect(host.sent).toHaveLength(1 + RESEND_COUNT_MAX);
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: false });
  });
});

describe("a write restored after a reload", () => {
  it("resends what may have committed at once, with the id and content it kept", async () => {
    for (const delivery of [{ state: "sending" }, { state: "unconfirmed", failure: { status: 0, message: "down" } }] as const) {
      const { host, view } = setup({ delivery, resubmitted: 0 }, [alreadyApplied]);
      await settled();
      expect(host.sent).toEqual([patch]);
      expect(view().state).toBe("landed");
    }
  });

  it("holds a rejection and a stop without sending anything", async () => {
    const rejection: Rejection = { rejection: "invalid", violations: [] };
    const held = setup({ delivery: { state: "rejected", rejection }, resubmitted: 0 });
    await held.advance(60_000);
    expect(held.view()).toMatchObject({ state: "rejected", rejection });
    const stopped = setup({ delivery: { state: "stopped" }, resubmitted: 0 });
    await stopped.advance(60_000);
    expect(stopped.view().state).toBe("stopped");
    expect(held.host.sent.length + stopped.host.sent.length).toBe(0);
  });

  it("keeps what it needs to be restored again: a record of each state that may matter", async () => {
    const { host, actor, view } = setup();
    expect(recordOf(view())).toBeUndefined();
    host.answers = [{ outcome: "rejected", rejection: { rejection: "patch_id_reused", patch_id: "p_one" } }];
    actor.send({ type: "submit" });
    await settled();
    expect(recordOf(view())).toMatchObject({ id: "p_one", about, delivery: { state: "rejected", rejection: { rejection: "patch_id_reused" } } });
    actor.send({ type: "discard" });
    expect(recordOf(view())).toBeUndefined();
  });
});

describe("what an answer makes of a write", () => {
  it("makes a 4xx answer to the first send a stop, not an uncertainty", async () => {
    const { host, actor, view, advance } = setup();
    host.answers = [unauthorized];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "stopped", failure: { status: 401 } });
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
  });

  it("makes a 5xx answer an uncertainty", async () => {
    const { host, actor, view } = setup();
    host.answers = [busy];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: true });
  });

  it("keeps a write unconfirmed when its resend is refused, since the lost attempt may have committed", async () => {
    const { host, actor, view, advance } = setup();
    host.answers = [lost, unauthorized];
    actor.send({ type: "submit" });
    await settled();
    await advance(backoffDelayMs(0));
    expect(view()).toMatchObject({ state: "unconfirmed", retrying: false, failure: { status: 401 } });
    expect(recordOf(view())?.delivery.state).toBe("unconfirmed");
  });

  it("keeps a rejection with what it tried to send, until it is discarded", async () => {
    const { host, actor, view } = setup();
    host.answers = [{ outcome: "rejected", rejection: { rejection: "invalid", violations: [] } }];
    actor.send({ type: "submit" });
    await settled();
    expect(view()).toMatchObject({ state: "rejected", command: { patch } });
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toHaveLength(1);
    actor.send({ type: "discard" });
    expect(actor.getSnapshot().status).toBe("done");
  });
});

describe("version skew", () => {
  it("sends nothing and stops the write before dispatch", async () => {
    const { host, switches, actor, view } = setup();
    switches.compatible = false;
    actor.send({ type: "submit" });
    await settled();
    expect(host.sent).toEqual([]);
    expect(view()).toMatchObject({ state: "stopped", failure: undefined });
    actor.send({ type: "retry" });
    await settled();
    expect(view().state).toBe("stopped");
    switches.compatible = true;
    host.answers = [applied];
    actor.send({ type: "retry" });
    await settled();
    expect(view().state).toBe("landed");
  });

  it("gives up waiting at once, without a resend, and does not rebase a stale answer", async () => {
    const waiting = setup();
    waiting.switches.connected = false;
    waiting.host.answers = [lost];
    waiting.actor.send({ type: "submit" });
    await settled();
    await waiting.advance(backoffDelayMs(0));
    waiting.switches.compatible = false;
    waiting.actor.send({ type: "wake" });
    expect(waiting.view()).toMatchObject({ state: "unconfirmed", retrying: false });

    const rebasing = setup();
    const held = deferred<Answered<HttpFailure>>();
    rebasing.host.answers = [held.promise];
    rebasing.actor.send({ type: "submit" });
    await settled();
    rebasing.switches.compatible = false;
    held.resolve(stale(5));
    await settled();
    expect(rebasing.host.sent).toHaveLength(1);
    expect(rebasing.view().state).toBe("rejected");
  });
});

describe("shutdown", () => {
  it("leaves no timer behind: a write in backoff does not resend after it is stopped", async () => {
    const { host, actor, advance } = setup();
    host.answers = [lost, applied];
    actor.send({ type: "submit" });
    await settled();
    actor.stop();
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
  });
});
