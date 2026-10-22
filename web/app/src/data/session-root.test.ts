// ARCHITECTURE, Web UI: the session root. Restoring the kept edits once it runs, version skew
// stopping new work, what the sync chip is told (the saves, the writes in flight), and shutdown
// leaving nothing running.
import { backoffDelayMs, type Answered, type HttpFailure, type Rejection, type StreamEvent } from "@cairn/client";
import { createActor, fromCallback } from "xstate";
import { describe, expect, it } from "vitest";

import { SAVE_COUNT_MAX } from "./activity.ts";
import { aboutNode, chipOf, renaming, startRoot } from "./edits.test-support.ts";
import { problemsOf } from "./edit-manager.ts";
import { EDIT_RECORD_VERSION, tabEditStore, type EditRecord } from "./edit-record.ts";
import { FakeHost, deferred, memoryStorage, settled } from "./fake.test-support.ts";
import { syncOf } from "./session-root.ts";
import { SYNC_SAVED_MS, SYNC_SHOW_AFTER_MS } from "./sync.ts";
import { writeView, type WriteSnapshot } from "./write-machine.ts";

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };
const invalid: Rejection = { rejection: "invalid", violations: [] };
const stale: Answered<HttpFailure> = { outcome: "rejected", rejection: { rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 5 }], intervening: [], touched: [] } };

const record = (id: string, delivery: EditRecord["delivery"], node = "n_a"): EditRecord => ({
  version: EDIT_RECORD_VERSION,
  id,
  command: { kind: "patch", patch: renaming(id, node) },
  about: aboutNode(node),
  delivery,
});

describe("restoring the kept edits", () => {
  it("resends what may have committed at once and holds a rejection, once the root is running", async () => {
    const storage = memoryStorage();
    const store = tabEditStore(() => storage);
    store.keep(record("p_sent", { state: "sending" }, "n_a"));
    store.keep(record("p_refused", { state: "rejected", rejection: invalid }, "n_b"));
    const fake = new FakeHost();
    fake.answers = [{ outcome: "answered", answer: { outcome: "already_applied", receipt } }];
    const { writes, root, saves } = await startRoot({ host: fake, storage, restore: false });
    // Nothing is started before the restore event: a write reads the root as it starts.
    expect(writes()).toEqual({});
    root.send({ type: "restore" });
    await settled();
    expect(fake.sent.map((patch) => patch.id)).toEqual(["p_sent"]);
    expect(saves).toHaveLength(1);
    expect(Object.keys(writes())).toEqual(["p_refused"]);
    expect(problemsOf(writes(), false).map((problem) => problem.id)).toEqual(["p_refused"]);
    expect(store.read().records.map((each) => each.id)).toEqual(["p_refused"]);
  });
});

describe("version skew", () => {
  it("stops a new write before it is sent, and says so", async () => {
    const { host, root, submit, writes } = await startRoot();
    root.send({ type: "skew" });
    submit(renaming("p_a"));
    await settled();
    expect(host.sent).toEqual([]);
    expect(writeView(writes().p_a?.snapshot as WriteSnapshot)).toMatchObject({ state: "stopped", failure: undefined });
    expect(problemsOf(writes(), true)).toEqual([expect.objectContaining({ id: "p_a", kind: "failed", retry: true, message: expect.stringContaining("Reload") as string })]);
  });

  it("gives up on a write waiting for the connection at once, without advancing time", async () => {
    const { host, switches, root, submit, advance, writes } = await startRoot();
    switches.connected = false;
    host.answers = [lost];
    submit(renaming("p_a"));
    await settled();
    await advance(backoffDelayMs(0));
    expect(writeView(writes().p_a?.snapshot as WriteSnapshot)).toMatchObject({ state: "unconfirmed", retrying: true });
    root.send({ type: "skew" });
    await settled();
    expect(writeView(writes().p_a?.snapshot as WriteSnapshot)).toMatchObject({ state: "unconfirmed", retrying: false });
    switches.connected = true;
    root.send({ type: "wake" });
    await advance(60_000);
    expect(host.sent).toHaveLength(1);
  });

  it("surfaces a stale answer instead of rebasing it", async () => {
    const { host, root, submit, writes } = await startRoot();
    const held = deferred<Answered<HttpFailure>>();
    host.answers = [held.promise, applied];
    submit(renaming("p_a"));
    await settled();
    root.send({ type: "skew" });
    held.resolve(stale);
    await settled();
    expect(host.sent).toHaveLength(1);
    expect(writeView(writes().p_a?.snapshot as WriteSnapshot).state).toBe("rejected");
  });
});

describe("what the sync chip is told", () => {
  const timing = (root: Parameters<typeof syncOf>[0]) => syncOf(root).getSnapshot().context.timing;

  it("keeps each landed write as a save, newest first and no more than the maximum, and says SAVED for a while", async () => {
    const { host, root, submit, advance } = await startRoot();
    host.answers = Array.from({ length: SAVE_COUNT_MAX + 1 }, () => applied);
    for (let write = 1; write <= SAVE_COUNT_MAX + 1; write += 1) {
      submit(renaming(`p_${String(write)}`));
      await settled();
    }
    const { saves } = root.getSnapshot().context.recent;
    expect(saves.map((save) => save.id)).toEqual(Array.from({ length: SAVE_COUNT_MAX }, (_, at) => SAVE_COUNT_MAX + 1 - at));
    expect(chipOf(root, { latest: saves[0] }).state).toBe("saved");
    await advance(SYNC_SAVED_MS);
    expect(chipOf(root).state).toBe("in-sync");
  });

  it("says SAVING for writes still out after 300ms, and not before", async () => {
    const { host, root, submit, advance } = await startRoot();
    host.answers = [deferred<Answered<HttpFailure>>().promise];
    submit(renaming("p_a"));
    await advance(SYNC_SHOW_AFTER_MS - 1);
    expect(chipOf(root).state).toBe("in-sync");
    await advance(1);
    expect(chipOf(root)).toMatchObject({ state: "saving", sentence: "Sending 1 change" });
  });

  it("leaves the scheduler as it is for a write that joins the writes in flight", async () => {
    const { host, root, submit, advance } = await startRoot();
    host.answers = [deferred<Answered<HttpFailure>>().promise, deferred<Answered<HttpFailure>>().promise];
    submit(renaming("p_a", "n_a"), aboutNode("n_a"));
    await advance(100);
    const before = timing(root);
    submit(renaming("p_b", "n_b"), aboutNode("n_b"));
    await settled();
    expect(timing(root)).toBe(before);
    await advance(SYNC_SHOW_AFTER_MS - 100);
    expect(chipOf(root)).toMatchObject({ state: "saving", sentence: "Sending 2 changes" });
  });

});

describe("the writes in flight the sync chip is told of", () => {
  it("counts a write sent after the others landed from its own start", async () => {
    const { host, root, submit, advance } = await startRoot();
    const first = deferred<Answered<HttpFailure>>();
    const second = deferred<Answered<HttpFailure>>();
    host.answers = [first.promise, second.promise];
    submit(renaming("p_a", "n_a"), aboutNode("n_a"));
    await advance(100);
    first.resolve(applied);
    await settled();
    await advance(50);
    submit(renaming("p_b", "n_b"), aboutNode("n_b"));
    await advance(SYNC_SHOW_AFTER_MS - 1);
    // Counted from the first write's start it would show SAVING by now.
    expect(chipOf(root).state).toBe("saved");
    await advance(1);
    expect(chipOf(root).state).toBe("saving");
  });

  it("counts a write restored in flight from the restore", async () => {
    const storage = memoryStorage();
    tabEditStore(() => storage).keep(record("p_sent", { state: "sending" }));
    const fake = new FakeHost();
    fake.answers = [deferred<Answered<HttpFailure>>().promise];
    const { root, advance } = await startRoot({ host: fake, storage, restore: false });
    await advance(1000);
    root.send({ type: "restore" });
    await settled();
    await advance(SYNC_SHOW_AFTER_MS - 1);
    expect(chipOf(root).state).toBe("in-sync");
    await advance(1);
    expect(chipOf(root).state).toBe("saving");
  });

  it("says NOT SAVED, not NEW VERSION, for a write that skew stopped", async () => {
    const { root, submit } = await startRoot();
    root.send({ type: "skew" });
    submit(renaming("p_a"));
    await settled();
    expect(chipOf(root).state).toBe("not-saved");
  });
});

describe("what the settled fact reads", () => {
  it("counts a read as work while it is out, and not once it has its answer", async () => {
    const host = new FakeHost();
    const answer = deferred<{ revision: number }>();
    host.deployment = () => answer.promise;
    const { root } = await startRoot({ host });
    expect(Object.keys(root.getSnapshot().context.working)).toHaveLength(1);
    answer.resolve({ revision: 1 });
    await settled();
    expect(root.getSnapshot().context.working).toEqual({});
  });

  it("tells the stream's watchers of a landed write's commit at once, as the stream's tick for it would", async () => {
    const { host, root, submit } = await startRoot();
    const heard: StreamEvent[] = [];
    const watcher = createActor(fromCallback<StreamEvent>(({ receive }) => {
      receive((event) => heard.push(event));
    })).start();
    root.send({ type: "watch", id: "probe", names: ["journey:j_one"], to: watcher });
    host.answers = [applied];
    submit(renaming("p_a"));
    await settled();
    expect(heard).toEqual([{ type: "stream.tick", tick: { of: { domain: { journey: "j_one" } }, revision: receipt.revision } }]);
  });
});

describe("shutdown", () => {
  it("leaves no timer, request or write running: one waiting for the connection, one in backoff, one with a request out", async () => {
    const { host, switches, root, edits, submit, advance, writes } = await startRoot();
    const held = deferred<Answered<HttpFailure>>();
    host.answers = [lost, lost, held.promise];
    switches.connected = false;
    submit(renaming("p_wait", "n_wait"), aboutNode("n_wait"));
    await advance(backoffDelayMs(0));
    submit(renaming("p_back", "n_back"), aboutNode("n_back"));
    submit(renaming("p_out", "n_out"), aboutNode("n_out"));
    await settled();
    const states = () => Object.values(writes()).map((entry) => writeView(entry.snapshot).state);
    expect(states()).toEqual(["unconfirmed", "unconfirmed", "sending"]);
    const sentBefore = host.sent.length;
    const refs = Object.values(writes()).map((entry) => entry.ref);
    root.stop();
    // A late answer a live write would send again, and a connection that is back.
    host.answers = [applied, applied, applied];
    switches.connected = true;
    held.resolve(stale);
    await advance(60_000);
    root.send({ type: "wake" });
    await advance(60_000);
    expect(host.sent).toHaveLength(sentBefore);
    expect([root, edits, ...refs].map((actor) => actor.getSnapshot().status)).toEqual(["stopped", "stopped", "stopped", "stopped", "stopped"]);
  });
});
