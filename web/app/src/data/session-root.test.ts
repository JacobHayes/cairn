// ARCHITECTURE, Web UI: the session root. Restoring the kept edits once it runs, version skew
// stopping new work, and shutdown leaving nothing running.
import { backoffDelayMs, type Answered, type HttpFailure, type Rejection } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { aboutNode, renaming, startRoot } from "./edits.test-support.ts";
import { problemsOf } from "./edit-manager.ts";
import { EDIT_RECORD_VERSION, tabEditStore, type EditRecord } from "./edit-record.ts";
import { FakeHost, deferred, memoryStorage, settled } from "./fake.test-support.ts";
import { writeView, type WriteSnapshot } from "./write-machine.ts";

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };
const invalid: Rejection = { rejection: "invalid", violations: [] };
const stale: Answered<HttpFailure> = { outcome: "rejected", rejection: { rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 5 }], intervening: [] } };

const record = (id: string, delivery: EditRecord["delivery"], node = "n_a"): EditRecord => ({
  version: EDIT_RECORD_VERSION,
  id,
  command: { kind: "patch", patch: renaming(id, node) },
  about: aboutNode(node),
  delivery,
  resubmitted: 0,
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
    // A late answer a live write would rebase and send again, and a connection that is back.
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
