// H5, H6: every write the tab sends is owned by the edit manager by its patch id, so writes
// from different views meet only through H5: one busy write per edit, an unrelated save leaves
// another write's problem alone, and a rejection waits to be dealt with across a reload.
import type { Answered, HttpFailure, Rejection } from "@cairn/client";
import { describe, expect, it } from "vitest";

import { aboutNode, renaming, startRoot } from "./edits.test-support.ts";
import { inFlightOf, problemsOf, unlandedWords } from "./edit-manager.ts";
import { EDIT_RECORD_VERSION, type EditRecord } from "./edit-record.ts";
import { FakeHost, deferred, memoryStorage, settled } from "./fake.test-support.ts";
import { RESEND_COUNT_MAX, writeView, type WriteSnapshot } from "./write-machine.ts";

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };
const invalid: Rejection = { rejection: "invalid", violations: [{ at: {}, code: "limit_exceeded", message: "The title is too long (F5)" }] };
const refusal = (rejection: Rejection): Answered<HttpFailure> => ({ outcome: "rejected", rejection });

describe("one busy write per edit", () => {
  it("sends once when a control submits twice before React could re-render", async () => {
    const { host, submit } = await startRoot();
    host.answers = [applied, applied];
    submit(renaming("p_first"));
    submit(renaming("p_second"));
    await settled();
    expect(host.sent.map((patch) => patch.id)).toEqual(["p_first"]);
  });

  it("lets another edit's write go out beside it", async () => {
    const { host, submit } = await startRoot();
    host.answers = [deferred<Answered<HttpFailure>>().promise, applied];
    submit(renaming("p_a", "n_a"), aboutNode("n_a"));
    submit(renaming("p_b", "n_b"), aboutNode("n_b"));
    await settled();
    expect(host.sent.map((patch) => patch.id)).toEqual(["p_a", "p_b"]);
  });

  it("lets a new submit replace a write that was refused, and keeps none of the old", async () => {
    const { host, storage, submit, writes } = await startRoot();
    host.answers = [refusal(invalid), applied];
    submit(renaming("p_old"));
    await settled();
    expect(Object.keys(writes())).toEqual(["p_old"]);
    submit(renaming("p_new"));
    await settled();
    expect(host.sent.map((patch) => patch.id)).toEqual(["p_old", "p_new"]);
    expect(writes()).toEqual({});
    expect(storage.length).toBe(0);
  });
});

describe("a write with no answer", () => {
  it("is not resolved by an unrelated save to the same journey, and stays until it is dealt with", async () => {
    const { host, edits, submit, advance, saves, writes } = await startRoot();
    host.answers = [lost, ...Array.from({ length: RESEND_COUNT_MAX }, () => lost)];
    submit(renaming("p_a", "n_a"), aboutNode("n_a"));
    await settled();
    // Resending, it is in flight and not yet a problem.
    expect(inFlightOf(writes())).toBe(1);
    expect(problemsOf(writes(), {}, false)).toEqual([]);
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      await advance(15_000);
    }
    expect(inFlightOf(writes())).toBe(0);
    expect(problemsOf(writes(), {}, false)).toEqual([expect.objectContaining({ id: "p_a", kind: "failed", retry: true, label: "Step n_a" })]);
    host.answers = [applied];
    submit(renaming("p_b", "n_b"), aboutNode("n_b"));
    await settled();
    expect(saves).toHaveLength(1);
    expect(problemsOf(writes(), {}, false).map((problem) => problem.id)).toEqual(["p_a"]);
    // Discard is an event sent to the write that owns the problem.
    edits.send({ type: "discard", id: "p_a" });
    await settled();
    expect(problemsOf(writes(), {}, false)).toEqual([]);
  });
});

describe("a rejection that arrives after the control is gone, then a reload", () => {
  it("waits on the chip with its label and address, and is dropped for good by Discard", async () => {
    const held = deferred<Answered<HttpFailure>>();
    const first = await startRoot();
    first.host.answers = [held.promise];
    first.submit(renaming("p_a"));
    await settled();
    // The control is long gone by the time the answer comes.
    held.resolve(refusal({ rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 0 }], intervening: [] }));
    await settled();
    first.root.stop();

    const second = await startRoot({ storage: first.storage });
    expect(problemsOf(second.writes(), {}, false)).toEqual([
      { id: "p_a", kind: "rejected", message: expect.stringMatching(/deleted/) as string, label: "Step n_a", address: undefined, retry: false },
    ]);
    expect(second.host.sent).toEqual([]);
    second.edits.send({ type: "discard", id: "p_a" });
    await settled();
    expect(problemsOf(second.writes(), {}, false)).toEqual([]);
    expect(first.storage.length).toBe(0);
  });

  it("keeps a stale rejection that could not be rebased with where it happened, as a conflict", async () => {
    const host = new FakeHost();
    // What intervened overlaps what the write touches, so the safe retry does not rebase it.
    host.overlaps = () => true;
    const first = await startRoot({ host });
    first.host.answers = [refusal({ rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 4 }], intervening: [] })];
    first.submit(renaming("p_a"));
    await settled();
    first.root.stop();
    const second = await startRoot({ storage: first.storage });
    expect(problemsOf(second.writes(), {}, false)).toEqual([
      expect.objectContaining({ id: "p_a", kind: "conflict", label: "Step n_a", address: "/journeys/j_one/n_a", retry: false }),
    ]);
  });
});

describe("the kept records", () => {
  const stored = (id: string): EditRecord => ({
    version: EDIT_RECORD_VERSION,
    id,
    command: { kind: "patch", patch: renaming(id) },
    about: aboutNode(),
    delivery: { state: "rejected", rejection: invalid },
    resubmitted: 0,
  });

  it("are dropped with one note when they are of another version, and nothing is started from them", async () => {
    const storage = memoryStorage();
    storage.setItem("cairn:edit:p_old", JSON.stringify({ ...stored("p_old"), version: EDIT_RECORD_VERSION + 1 }));
    const { notes, writes, host } = await startRoot({ storage });
    expect(notes.dropped).toEqual([1]);
    expect(writes()).toEqual({});
    expect(host.sent).toEqual([]);
    expect(storage.length).toBe(0);
  });

  it("make the tab say once that edits are kept only until reload when storage refuses them", async () => {
    const refusing = Object.assign(memoryStorage(), {
      setItem: () => {
        throw new DOMException("full", "QuotaExceededError");
      },
    });
    const { notes, host, submit } = await startRoot({ storage: refusing });
    host.answers = [refusal(invalid), refusal(invalid)];
    submit(renaming("p_a", "n_a"), aboutNode("n_a"));
    submit(renaming("p_b", "n_b"), aboutNode("n_b"));
    await settled();
    expect(notes.unstored).toBe(1);
  });

  it("are not written back by a snapshot a write relays after it was discarded", async () => {
    const { host, storage, edits, submit, writes } = await startRoot();
    host.answers = [refusal(invalid)];
    submit(renaming("p_a"));
    await settled();
    const relayed: WriteSnapshot = writes().p_a?.snapshot as WriteSnapshot;
    expect(storage.length).toBe(1);
    edits.send({ type: "discard", id: "p_a" });
    await settled();
    expect(storage.length).toBe(0);
    edits.send({ type: "xstate.snapshot.p_a", snapshot: relayed });
    await settled();
    expect(storage.length).toBe(0);
    expect(writes()).toEqual({});
  });

  it("are put back once, however often restore is asked", async () => {
    const storage = memoryStorage();
    storage.setItem("cairn:edit:p_a", JSON.stringify(stored("p_a")));
    const { edits, root, writes } = await startRoot({ storage });
    expect(Object.keys(writes())).toEqual(["p_a"]);
    const before = writes().p_a;
    root.send({ type: "restore" });
    edits.send({ type: "restore" });
    await settled();
    expect(writes().p_a).toBe(before);
  });
});

describe("what the chip says of a write", () => {
  const base = { id: "p_a", about: aboutNode(), command: { kind: "patch" as const, patch: renaming("p_a") }, resubmitted: 0 };
  const stale: Rejection = { rejection: "stale", conflicts: [], intervening: [] };

  it("calls a stale rejection a conflict and another refusal not saved, offers Retry only where a resend can help, and says nothing of a write still trying", () => {
    const words = (view: Parameters<typeof unlandedWords>[0], skewed = false) => {
      const found = unlandedWords(view, skewed);
      return found === undefined ? undefined : [found.kind, found.retry];
    };
    expect(words({ ...base, state: "rejected", rejection: stale })).toEqual(["conflict", false]);
    expect(words({ ...base, state: "rejected", rejection: invalid })).toEqual(["rejected", false]);
    expect(words({ ...base, state: "gone", removed: { node: "n_a" } })).toEqual(["rejected", false]);
    expect(words({ ...base, state: "stopped", failure: undefined }, true)).toEqual(["failed", true]);
    expect(words({ ...base, state: "stopped", failure: { status: 401, message: "sign in" } })).toEqual(["failed", true]);
    expect(words({ ...base, state: "unconfirmed", failure: undefined, retrying: false })).toEqual(["failed", true]);
    expect(words({ ...base, state: "unconfirmed", failure: undefined, retrying: true })).toBeUndefined();
    expect(words({ ...base, state: "sending" })).toBeUndefined();
  });

  it("says what was removed, and offers no way back to it", async () => {
    const { host, submit, writes } = await startRoot();
    host.answers = [refusal({ rejection: "stale", conflicts: [{ of: { domain: { journey: "j_one" } }, expected: 3, current: 0 }], intervening: [] })];
    submit(renaming("p_a"));
    await settled();
    const view = writeView(writes().p_a?.snapshot as WriteSnapshot);
    expect(view).toMatchObject({ state: "gone", removed: { journey: "j_one" } });
    expect(problemsOf(writes(), {}, false)[0]).toMatchObject({ kind: "rejected", address: undefined });
  });

  it("lists the writes that need the person in the order they were sent, then the problems a control still reports", async () => {
    const { host, submit, edits, writes } = await startRoot();
    host.answers = [refusal(invalid), refusal(invalid)];
    submit(renaming("p_a", "n_a"), aboutNode("n_a"));
    submit(renaming("p_b", "n_b"), aboutNode("n_b"));
    await settled();
    let discarded = 0;
    edits.send({ type: "report", key: "screen:1", problem: { kind: "rejected", message: "no", label: undefined, address: undefined }, discard: () => { discarded += 1; } });
    const { reported } = edits.getSnapshot().context;
    expect(problemsOf(writes(), reported, false).map((problem) => problem.id)).toEqual(["p_a", "p_b", "screen:1"]);
    edits.send({ type: "discard", id: "screen:1" });
    expect(discarded).toBe(1);
    expect(problemsOf(writes(), edits.getSnapshot().context.reported, false)).toHaveLength(2);
  });
});

describe("a new write's record", () => {
  it("is kept by the time its request goes out", async () => {
    const { host, storage, submit } = await startRoot();
    const kept: string[][] = [];
    const answer = deferred<Answered<HttpFailure>>();
    host.answers = [answer.promise];
    const send = host.send.bind(host);
    host.send = (patch, ...rest) => {
      kept.push(Array.from({ length: storage.length }, (_, at) => storage.key(at) ?? ""));
      return send(patch, ...rest);
    };
    submit(renaming("p_a"));
    await settled();
    expect(kept).toEqual([["cairn:edit:p_a"]]);
    answer.resolve(applied);
  });
});
