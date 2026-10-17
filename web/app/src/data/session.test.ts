// The session over a fake host: what a landed write records, how a control waits on a write,
// and the wiring of the connection to the manager's waiting writes (H5).
import { backoffDelayMs, type Answered, type HttpFailure, type Rejection, type RevisionOf } from "@cairn/client";
import { SimulatedClock, createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { aboutNode, chipOf, renaming } from "./edits.test-support.ts";
import { problemsOf, submitWrite } from "./edit-manager.ts";
import { EDIT_RECORD_VERSION, tabEditStore } from "./edit-record.ts";
import { HandHost, replied, target } from "../assistant/hand-host.test-support.ts";
import { sendTurn } from "../assistant/conversation.ts";
import { CountingClock, FakeDeriver, FakeHost, deferred, memoryStorage, settled } from "./fake.test-support.ts";
import { ReadFailed } from "./host.ts";
import { HOST_READ, keyedRead, type ReadSource } from "./keyed-read.ts";
import { Session } from "./session.ts";
import { toastOf } from "./toasts.ts";
import { RESEND_COUNT_MAX } from "./write-machine.ts";
import { landedWarning, patchOf, type WriteIntent } from "./writes.ts";

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
  const session = await Session.start({ host, deriver: new FakeDeriver(), clock, monotonic: () => clock.now(), store: tabEditStore(() => storage) });
  // The stream opens once the batch of watches is reconciled.
  clock.increment(0);
  host.open();
  await settled();
  const advance = async (ms: number) => {
    clock.increment(ms);
    await settled();
  };
  return { host, session, storage, advance };
}

const problems = (session: Session) => problemsOf(session.edits.getSnapshot().context.writes, session.skewed);

/** Sends the test's patch as the edit `title:j_one:n_a` and waits as a control does. */
const send = (session: Session) => submitWrite(session.edits, { id: "p_one", command: { kind: "patch", patch: patchOf(intent, "p_one") }, about: aboutNode() });

describe("a control waiting on its write", () => {
  it("is answered with the landed write, whose save carries the warning the receipt shows, and nothing is held", async () => {
    const { host, session } = await started();
    host.answers = [applied];
    const answer = await send(session);
    expect(answer).toMatchObject({ outcome: "applied" });
    const warning = answer === undefined ? undefined : landedWarning(answer, session.titleOf);
    expect(warning).toBe(session.root.getSnapshot().context.recent.saves[0]?.warning);
    expect(warning).toBeDefined();
    expect(session.edits.getSnapshot().context.writes).toEqual({});
    expect(chipOf(session.root).state).toBe("saved");
  });

  it("leaves a drafted proposal's link in Recent even when nobody was waiting for the write", async () => {
    const { host, session } = await started();
    host.proposed = [{ outcome: "answered", answer: { outcome: "already_saved" } as never }];
    session.edits.send({
      type: "submit",
      id: "p_one",
      command: { kind: "proposal", call: { op: "upgrade", journey: "j_one", request: { patch_id: "p_one", proposal: "pr_new", to: 2 } } },
      about: aboutNode(),
    });
    await settled();
    expect(session.root.getSnapshot().context.recent.saves.map((save) => save.address)).toEqual(["/journeys/j_one/proposals/pr_new"]);
    expect(session.edits.getSnapshot().context.writes).toEqual({});
  });

  it("is answered none for a rejection, which the manager keeps, counts on the chip, and stores", async () => {
    const { host, session, storage } = await started();
    const rejection: Rejection = { rejection: "invalid", violations: [] };
    host.answers = [{ outcome: "rejected", rejection }];
    expect(await send(session)).toBeUndefined();
    await settled();
    expect(Object.keys(session.edits.getSnapshot().context.writes)).toEqual(["p_one"]);
    expect(problems(session)).toEqual([expect.objectContaining({ id: "p_one", kind: "rejected" })]);
    expect(storage.length).toBe(1);
    expect(chipOf(session.root).state).toBe("not-saved");
  });

  it("stays pending while a lost answer is resent, then lands on already_applied", async () => {
    const { host, session, advance } = await started();
    host.answers = [lost, alreadyApplied];
    let result: unknown = "waiting";
    void send(session).then((each) => { result = each; });
    await settled();
    expect(result).toBe("waiting");
    await advance(backoffDelayMs(0));
    expect(result).toMatchObject({ outcome: "already_applied" });
    expect(host.sent.map((patch) => patch.id)).toHaveLength(2);
    expect(new Set(host.sent.map((patch) => patch.id)).size).toBe(1);
    expect(problems(session)).toEqual([]);
  });

  it("is answered none once the resends are used up, and leaves one problem with Retry", async () => {
    const { host, session, advance } = await started();
    host.answers = Array.from({ length: RESEND_COUNT_MAX + 1 }, () => lost);
    let result: unknown = "waiting";
    void send(session).then((each) => { result = each; });
    await settled();
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      await advance(15_000);
    }
    expect(result).toBeUndefined();
    expect(problems(session)).toEqual([expect.objectContaining({ kind: "failed", retry: true })]);
    expect(chipOf(session.root).state).toBe("not-saved");
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
    const clock = new SimulatedClock();
    const session = await Session.start({ host, deriver: new FakeDeriver(), clock, monotonic: () => clock.now(), store: tabEditStore(() => storage) });
    clock.increment(0);
    await settled();
    expect(host.sent).toEqual([]);
    host.open();
    await settled();
    expect(host.sent.map((patch) => patch.id)).toEqual(["p_kept"]);
    expect(storage.length).toBe(0);
    expect(session.root.getSnapshot().context.recent.saves).toHaveLength(1);
  });

  it("is not listened to once the session is closed", async () => {
    const held = deferred<Answered<HttpFailure>>();
    const { host, session, advance } = await started();
    host.answers = [held.promise];
    void send(session);
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

describe("a read the sync chip counts", () => {
  const route: RevisionOf = { domain: { route: "r_a" } };
  /** A read of a route that reports its lag, answering the revision the test sets or failing while it is told to. */
  function routeRead() {
    const state = { revision: 1, failing: false };
    const source: ReadSource<undefined, { revision: number }> = {
      policy: HOST_READ,
      read: () => (state.failing ? Promise.reject(new ReadFailed({ status: 500, message: "the server broke" })) : Promise.resolve({ revision: state.revision })),
      ticks: () => ({ watching: ["route:r_a"], about: (of) => JSON.stringify(of) === JSON.stringify(route), holds: (value) => [{ of: route, revision: value.revision }], subject: route, reportsLag: true }),
    };
    return { state, source };
  }

  it("turns the chip BEHIND when its refetch fails, keeps what it shows, and clears once the read ends", async () => {
    const { host, session, advance } = await started();
    const { state, source } = routeRead();
    const clock = new SimulatedClock();
    const actor = createActor(keyedRead, { clock, input: { source, env: session.readEnv } }).start();
    actor.send({ type: "want", key: { identity: ["r_a"], freshness: [], args: undefined } });
    await advance(0);
    host.open();
    await settled();
    expect(chipOf(session.root).state).not.toBe("behind");
    state.failing = true;
    host.tick({ of: route, revision: 2 });
    await settled();
    expect(chipOf(session.root).state).toBe("behind");
    expect(actor.getSnapshot().context.held?.value).toEqual({ revision: 1 });
    // The server recovers: the retry lands the newer revision and the chip clears.
    state.failing = false;
    state.revision = 2;
    clock.increment(backoffDelayMs(0));
    await settled();
    expect(actor.getSnapshot().context.held?.value).toEqual({ revision: 2 });
    expect(chipOf(session.root).state).not.toBe("behind");
    actor.stop();
  });

  it("clears the chip when the screen that showed the read goes away", async () => {
    const { host, session, advance } = await started();
    const { state, source } = routeRead();
    const actor = createActor(keyedRead, { input: { source, env: session.readEnv } }).start();
    actor.send({ type: "want", key: { identity: ["r_a"], freshness: [], args: undefined } });
    await advance(0);
    host.open();
    await settled();
    state.failing = true;
    host.tick({ of: route, revision: 2 });
    await settled();
    expect(chipOf(session.root).state).toBe("behind");
    actor.stop();
    await settled();
    expect(chipOf(session.root).state).not.toBe("behind");
  });
});

describe("the caller", () => {
  const deployment: RevisionOf = { domain: "deployment" };

  it("is read once even when the deployment could not be read, and again for each newer deployment held", async () => {
    const host = new FakeHost();
    host.deploymentFails = true;
    const clock = new SimulatedClock();
    const session = await Session.start({ host, deriver: new FakeDeriver(), clock, monotonic: () => clock.now(), store: tabEditStore(() => memoryStorage()) });
    clock.increment(0);
    host.open();
    await settled();
    expect(host.viewerFetches).toBe(1);
    // The deployment recovers at a newer revision: it is held, and the caller is read for it.
    host.deploymentFails = false;
    host.deploymentRevision = 2;
    host.tick({ of: deployment, revision: 2 });
    await settled();
    expect(session.deployment.getSnapshot().context.held?.value).toEqual({ revision: 2 });
    expect(host.viewerFetches).toBe(2);
    // The same revision again asks for nothing.
    host.tick({ of: deployment, revision: 2 });
    await settled();
    expect(host.viewerFetches).toBe(2);
    session.close();
  });
});

describe("the chip and the toast", () => {
  it("says NEW VERSION once the tab is in version skew", async () => {
    const { session } = await started();
    expect(chipOf(session.root).state).toBe("in-sync");
    session.root.send({ type: "skew" });
    expect(chipOf(session.root).state).toBe("new-version");
  });

  it("raises a problem toast for the unsent edits of an earlier version it could not keep", async () => {
    const storage = memoryStorage();
    storage.setItem("cairn:edit:p_old", JSON.stringify({ version: EDIT_RECORD_VERSION + 1, id: "p_old" }));
    const { session } = await started(storage);
    expect(toastOf(session.toasts.getSnapshot())).toMatchObject({ tone: "problem", text: expect.stringContaining("unsent edit") as string });
  });

  it("leaves no timer or turn running once closed: a toast shown, SAVED pending, and a turn out", async () => {
    const assistant = new HandHost();
    const host = new FakeHost();
    host.journeysHeld.set("j_one", { revision: 3, deployment: 1, today: "2026-10-06" });
    host.offersAssistant = true;
    host.assistant = assistant;
    const clock = new CountingClock();
    const storage = memoryStorage();
    const session = await Session.start({ host, deriver: new FakeDeriver(), clock, monotonic: () => clock.now(), store: tabEditStore(() => storage) });
    clock.increment(0);
    host.open();
    await settled();
    host.answers = [applied];
    await send(session);
    session.toasts.send({ type: "show", text: "Route file exported", tone: "confirm" });
    session.conversations.send({ type: "open", target });
    await settled();
    const turn = session.conversations.getSnapshot().context.conversations["journey:j_one"];
    if (turn === undefined) {
      throw new Error("the conversation was not started");
    }
    const sent = sendTurn(turn, "Hello?");
    session.close();
    expect(clock.pending).toEqual([]);
    assistant.turns[0]?.answer(replied("Hi."));
    await settled();
    expect(await sent).toBe(false);
    expect(assistant.reads).toHaveLength(1);
  });
});
