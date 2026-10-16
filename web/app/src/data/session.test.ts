// The session over a fake host: the write adapter the controls not yet on the edit manager
// still use, and the wiring of the connection to the manager's waiting writes (H5).
import { backoffDelayMs, type Answered, type HttpFailure, type Rejection } from "@cairn/client";
import { SimulatedClock } from "xstate";
import { describe, expect, it } from "vitest";

import { aboutNode, renaming } from "./edits.test-support.ts";
import { problemsOf } from "./edit-manager.ts";
import { EDIT_RECORD_VERSION, tabEditStore } from "./edit-record.ts";
import { FakeDeriver, FakeHost, deferred, manualTimers, memoryStorage, settled } from "./fake.test-support.ts";
import { Session } from "./session.ts";
import { RESEND_COUNT_MAX } from "./write-machine.ts";
import type { WriteIntent } from "./writes.ts";

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: { j_one: { overdue: ["n_b"] } } } };
const alreadyApplied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "already_applied", receipt } };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };

const intent: WriteIntent = {
  target: { journey: "j_one" },
  baseRevision: 3,
  mutations: [{ op: "set_node_field", node: "n_a", value: { title: "Renamed" } }],
};

async function started(storage = memoryStorage()) {
  const host = new FakeHost();
  host.journeysHeld.set("j_one", { revision: 3, deployment: 1, today: "2026-10-06" });
  const clock = new SimulatedClock();
  const { timers } = manualTimers();
  const session = await Session.start({ host, deriver: new FakeDeriver(), timers, clock, store: tabEditStore(() => storage) });
  host.open();
  await settled();
  const advance = async (ms: number) => {
    clock.increment(ms);
    await settled();
  };
  return { host, session, storage, advance };
}

const problems = (session: Session) => {
  const { writes, reported } = session.edits.getSnapshot().context;
  return problemsOf(writes, reported, session.skewed);
};

describe("session.write", () => {
  it("answers a landed write with its warning and leaves nothing held", async () => {
    const { host, session } = await started();
    host.answers = [applied];
    const result = await session.write(intent);
    expect(result).toMatchObject({ outcome: "landed", resubmitted: 0 });
    expect(result.outcome === "landed" ? result.warning : undefined).toBe(session.activity.saves[0]?.warning);
    expect(result.outcome === "landed" ? result.warning : undefined).toBeDefined();
    expect(session.edits.getSnapshot().context.writes).toEqual({});
    expect(session.sync.summary.state).toBe("saved");
  });

  it("answers a rejection to the control that reports it, and keeps no write or problem of its own", async () => {
    const { host, session, storage } = await started();
    const rejection: Rejection = { rejection: "invalid", violations: [] };
    host.answers = [{ outcome: "rejected", rejection }];
    expect(await session.write(intent)).toMatchObject({ outcome: "rejected", rejection });
    await settled();
    expect(session.edits.getSnapshot().context.writes).toEqual({});
    expect(problems(session)).toEqual([]);
    expect(storage.length).toBe(0);
  });

  it("stays pending while a lost answer is resent, then lands on already_applied", async () => {
    const { host, session, advance } = await started();
    host.answers = [lost, alreadyApplied];
    let result: unknown;
    void session.write(intent).then((each) => { result = each; });
    await settled();
    expect(result).toBeUndefined();
    await advance(backoffDelayMs(0));
    expect(result).toMatchObject({ outcome: "landed" });
    expect(host.sent.map((patch) => patch.id)).toHaveLength(2);
    expect(new Set(host.sent.map((patch) => patch.id)).size).toBe(1);
    expect(problems(session)).toEqual([]);
  });

  it("answers failed once the resends are used up, and leaves one problem with Retry", async () => {
    const { host, session, advance } = await started();
    host.answers = Array.from({ length: RESEND_COUNT_MAX + 1 }, () => lost);
    let result: unknown;
    void session.write(intent).then((each) => { result = each; });
    await settled();
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      await advance(15_000);
    }
    expect(result).toMatchObject({ outcome: "failed", failure: { status: 0 } });
    expect(problems(session)).toEqual([expect.objectContaining({ kind: "failed", retry: true })]);
    expect(session.sync.summary.state).toBe("not-saved");
  });
});

describe("the connection", () => {
  it("lets a restored write wait until the stream is open, then resends it", async () => {
    const storage = memoryStorage();
    tabEditStore(() => storage).keep({
      version: EDIT_RECORD_VERSION,
      id: "p_kept",
      command: { kind: "patch", patch: renaming("p_kept") },
      about: aboutNode(),
      delivery: { state: "sending" },
      resubmitted: 0,
    });
    const host = new FakeHost();
    host.answers = [alreadyApplied];
    const session = await Session.start({ host, deriver: new FakeDeriver(), timers: manualTimers().timers, clock: new SimulatedClock(), store: tabEditStore(() => storage) });
    await settled();
    expect(host.sent).toEqual([]);
    host.open();
    await settled();
    expect(host.sent.map((patch) => patch.id)).toEqual(["p_kept"]);
    expect(storage.length).toBe(0);
    expect(session.activity.saves).toHaveLength(1);
  });

  it("is not listened to once the session is closed", async () => {
    const held = deferred<Answered<HttpFailure>>();
    const { host, session, advance } = await started();
    host.answers = [held.promise];
    void session.write(intent);
    await settled();
    session.close();
    held.resolve(lost);
    await advance(60_000);
    host.open();
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(session.root.getSnapshot().status).toBe("stopped");
  });
});
