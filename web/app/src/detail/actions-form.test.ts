// D1, D4, B6, H5: a node's actions hold one open form, saved as one draft, which closes with the
// end of the write it sent only when nothing was typed since. The real session root and edit
// manager run over a fake host whose answers the tests hold and release.
import type { Answered, HttpFailure, Rejection } from "@cairn/client";
import { createActor } from "xstate";
import { afterEach, describe, expect, it, vi } from "vitest";

import { aboutNode, renaming, startRoot, type StartOptions } from "../data/edits.test-support.ts";
import { submitPatch } from "../data/edit-manager.ts";
import { RESEND_COUNT_MAX } from "../data/write-machine.ts";
import { deferred, memoryStorage, settled } from "../data/fake.test-support.ts";
import { readDraft, writeDraft } from "../data/drafts.ts";
import { actionsDraftKey, actionsForm, formView, isDrafted, opening, readOpened, takeOpened, type Drafted, type Opened } from "./actions-form.ts";

const receipt = { patch_id: "p_x", domain: { journey: "j_one" }, content_hash: "h", revision: 4 };
const applied: Answered<HttpFailure> = { outcome: "answered", answer: { outcome: "applied", receipt, consequences: {} } };
const lost: Answered<HttpFailure> = { outcome: "failed", error: { status: 0, message: "network down" } };
const invalid: Rejection = { rejection: "invalid", violations: [{ at: {}, code: "limit_exceeded", message: "The reason is too long" }] };
const refusal: Answered<HttpFailure> = { outcome: "rejected", rejection: invalid };

const SEEN = { base: 3, deployment: 1 };
const ABOUT = { ...aboutNode(), edit: "node:j_one:actions:n_a" };

afterEach(() => {
  vi.unstubAllGlobals();
});

/** A running root, and a form machine over its edit manager that keeps its drafts in a list. */
async function started(options: StartOptions & { restored?: Opened | undefined } = {}) {
  const root = await startRoot(options);
  const saved: (Opened | undefined)[] = [];
  const begin = (restored: Opened | undefined) => {
    const actor = createActor(actionsForm, { input: { edits: root.edits, restored, save: (opened) => saved.push(opened) } });
    actor.start();
    return actor;
  };
  const actor = begin(options.restored);
  /** What a control does on submit: the write goes to the manager, and the form is told when it was accepted. */
  const submit = (id: string, version = drafted(actor).version) => {
    const accepted = submitPatch(root.edits, renaming(id), ABOUT) !== undefined;
    if (accepted) {
      actor.send({ type: "sent", id, version });
    }
    return accepted;
  };
  return { ...root, actor, saved, begin, submit, last: () => saved.at(-1) };
}

function drafted(actor: { getSnapshot(): { context: { opened: Opened | undefined } } }): Drafted {
  const { opened } = actor.getSnapshot().context;
  if (!isDrafted(opened)) {
    throw new Error("no form is open on a draft");
  }
  return opened;
}

describe("readOpened keeps what a form can hold", () => {
  const forms: Opened[] = [
    opening("skip", "Waiting on the vendor", SEEN),
    opening("keep", "", SEEN),
    opening("include", "Needed", SEEN),
    opening("bypass", "Reviewed", SEEN),
    opening("bypass", "", SEEN),
    opening("snooze", { aside: "n_a", until: "date", date: "2026-10-20", target: "" }, SEEN),
    opening("evidence", { artifact: "https://example.test", note: "Done" }, SEEN),
    opening("edit-date", "2026-10-07", SEEN),
    { ...opening("skip", "Sent", SEEN), version: 2, sent: { id: "p_1", version: 1 } },
    { ...opening("skip", "Sent and typed since", SEEN), version: 2, kept: true },
    { case: "assign" },
    { case: "weight" },
    { case: "break-down" },
  ];
  it.each(forms.map((opened) => [opened.case + (isDrafted(opened) && opened.sent !== undefined ? " sent" : isDrafted(opened) && opened.kept === true ? " kept" : ""), opened] as const))("keeps %s through storage", (_, opened) => {
    expect(readOpened(JSON.parse(JSON.stringify(opened)))).toEqual(opened);
  });
});

describe("readOpened refuses what no form can hold", () => {
  const good = opening("skip", "Why", SEEN);
  const bad: [string, unknown][] = [
    ["an unknown case", { ...good, case: "explode" }],
    ["a reason that is not text", { ...good, value: 4 }],
    ["a snooze until a time the form does not offer", { ...opening("snooze", { aside: "n_a", until: "tomorrow", date: "", target: "" }, SEEN), value: { aside: "n_a", until: "never", date: "", target: "" } }],
    ["evidence as a bare string", { ...opening("evidence", { artifact: "", note: "" }, SEEN), value: "https://example.test" }],
    ["a negative version", { ...good, version: -1 }],
    ["a fractional version", { ...good, version: 1.5 }],
    ["a write sent at a version the draft has not reached", { ...good, version: 1, sent: { id: "p_1", version: 2 } }],
    ["a write without an id", { ...good, version: 1, sent: { version: 1 } }],
    ["a kept flag that is not true", { ...good, kept: false }],
    ["a kept form that is also out", { ...good, version: 1, kept: true, sent: { id: "p_1", version: 1 } }],
    ["no revisions", { case: "skip", value: "Why", version: 0 }],
    ["a fractional revision", { ...good, seen: { base: 3.5, deployment: 1 } }],
    ["no object at all", "skip"],
  ];
  it.each(bad)("refuses %s", (_, value) => {
    expect(readOpened(value)).toBeUndefined();
  });

  it("drops what it does not know of a draft", () => {
    expect(readOpened({ ...good, stray: true })).toEqual(good);
  });
});

describe("takeOpened", () => {
  it("returns the kept draft, and removes one that fails validation so nothing opens", () => {
    vi.stubGlobal("sessionStorage", memoryStorage());
    const key = actionsDraftKey("j_one", "n_a");
    writeDraft(key, opening("skip", "Why", SEEN));
    expect(takeOpened(key)).toEqual(opening("skip", "Why", SEEN));
    expect(readDraft(key)).toBeDefined();
    writeDraft(key, { case: "skip", value: 7 });
    expect(takeOpened(key)).toBeUndefined();
    expect(readDraft(key)).toBeUndefined();
  });
});

describe("one form at a time", () => {
  it("replaces the open form when another opens, and keeps one draft", async () => {
    const { actor, last } = await started();
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    actor.send({ type: "open", opened: opening("snooze", { aside: "n_a", until: "tomorrow", date: "2026-10-07", target: "" }, SEEN) });
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "editing", opened: { case: "snooze" } });
    expect(last()).toMatchObject({ case: "snooze" });
    // A machine restored from the saved draft opens that one form.
    const restored = createActor(actionsForm, { input: { edits: actor.getSnapshot().context.edits, restored: readOpened(JSON.parse(JSON.stringify(last()))), save: () => undefined } });
    restored.start();
    expect(formView(restored.getSnapshot())).toMatchObject({ state: "editing", opened: { case: "snooze" } });
  });

  it("keeps what was typed when the case already open is chosen again, and opens a panel in the form's place", async () => {
    const { actor } = await started();
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "Typed" });
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    expect(drafted(actor)).toMatchObject({ value: "Typed", version: 1 });
    actor.send({ type: "open", opened: { case: "assign" } });
    expect(formView(actor.getSnapshot())).toEqual({ state: "editing", opened: { case: "assign" } });
    actor.send({ type: "change", case: "skip", value: "ignored" });
    expect(formView(actor.getSnapshot())).toEqual({ state: "editing", opened: { case: "assign" } });
    actor.send({ type: "close" });
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
  });

  it("counts a change of the draft up, and ignores a change meant for another case", async () => {
    const { actor } = await started();
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    actor.send({ type: "change", case: "keep", value: "b" });
    actor.send({ type: "change", case: "skip", value: "ab" });
    expect(drafted(actor)).toMatchObject({ value: "ab", version: 2 });
  });
});

describe("the end of the write a form sent", () => {
  it("closes the form and forgets the draft when nothing was typed since", async () => {
    const { actor, host, submit, last } = await started();
    const answer = deferred<Answered<HttpFailure>>();
    host.answers = [answer.promise];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "Waiting" });
    expect(submit("p_1")).toBe(true);
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "sending", sent: { id: "p_1", version: 1 } });
    answer.resolve(applied);
    await settled();
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
    expect(last()).toBeUndefined();
  });

  it("keeps text typed after the submit when the write lands, read-only", async () => {
    const { actor, host, submit, last } = await started();
    const answer = deferred<Answered<HttpFailure>>();
    host.answers = [answer.promise];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    actor.send({ type: "change", case: "skip", value: "ab" });
    answer.resolve(applied);
    await settled();
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "kept", opened: { value: "ab" } });
    expect(drafted(actor).sent).toBeUndefined();
    expect(last()).toMatchObject({ value: "ab", kept: true });
    actor.send({ type: "change", case: "skip", value: "abc" });
    expect(drafted(actor).value).toBe("ab");
    actor.send({ type: "close" });
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
    expect(last()).toBeUndefined();
  });

  it("claims no success when the write was discarded after text was typed: the form stays editable", async () => {
    const { actor, host, edits, submit, writes } = await started();
    host.answers = [Promise.resolve(refusal)];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    await settled();
    // A rejection does not end the write: the form stays with it.
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "sending" });
    actor.send({ type: "change", case: "skip", value: "ab" });
    edits.send({ type: "discard", id: "p_1" });
    await settled();
    expect(writes()).toEqual({});
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "editing", opened: { value: "ab" } });
    expect(drafted(actor).kept).toBeUndefined();
    expect(drafted(actor).sent).toBeUndefined();
  });

});

describe("the end of the write a form sent, when it was not a landing", () => {
  it("closes the form when the write it sent is discarded with nothing typed since", async () => {
    const { actor, host, edits, submit, writes } = await started();
    host.answers = [Promise.resolve(refusal)];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    await settled();
    edits.send({ type: "discard", id: "p_1" });
    await settled();
    expect(writes()).toEqual({});
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
  });

  it("leaves the form alone when another edit's write lands", async () => {
    const { actor, host, submit, edits } = await started();
    host.answers = [deferred<Answered<HttpFailure>>().promise, applied];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    submitPatch(edits, renaming("p_other", "n_b"), aboutNode("n_b"));
    await settled();
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "sending", sent: { id: "p_1" } });
  });
});

describe("closing a form", () => {
  it("never discards a write that is out: it still lands, and the form stays closed", async () => {
    const { actor, host, submit, writes, last } = await started();
    const answer = deferred<Answered<HttpFailure>>();
    host.answers = [answer.promise];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    actor.send({ type: "close" });
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
    expect(last()).toBeUndefined();
    expect(Object.keys(writes())).toEqual(["p_1"]);
    answer.resolve(applied);
    await settled();
    expect(host.sent.map((patch) => patch.id)).toEqual(["p_1"]);
    expect(writes()).toEqual({});
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
  });

  it("discards the write it sent once that was rejected", async () => {
    const { actor, host, submit, writes } = await started();
    host.answers = [Promise.resolve(refusal)];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    await settled();
    expect(Object.keys(writes())).toEqual(["p_1"]);
    actor.send({ type: "close" });
    await settled();
    expect(writes()).toEqual({});
  });

  it("keeps a write that may have committed when no answer came and it stopped resending", async () => {
    const { actor, host, submit, writes, advance } = await started();
    host.answers = Array.from({ length: RESEND_COUNT_MAX + 1 }, () => lost);
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    await settled();
    for (let resend = 0; resend < RESEND_COUNT_MAX; resend += 1) {
      await advance(15_000);
    }
    expect(writes()["p_1"]?.snapshot.hasTag("needsYou")).toBe(true);
    actor.send({ type: "close" });
    await settled();
    expect(Object.keys(writes())).toEqual(["p_1"]);
  });
});

describe("a rejection and the retry that follows", () => {
  it("stays open on the rejected write, then closes on the landing of the edited version's new write", async () => {
    const { actor, host, submit, writes } = await started();
    const second = deferred<Answered<HttpFailure>>();
    host.answers = [Promise.resolve(refusal), second.promise];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    await settled();
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "sending", sent: { id: "p_1" } });
    // The author edits the reason and sends again: the manager drops the rejected write for the new one.
    actor.send({ type: "change", case: "skip", value: "ab" });
    expect(submit("p_2")).toBe(true);
    await settled();
    expect(Object.keys(writes())).toEqual(["p_2"]);
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "sending", sent: { id: "p_2", version: 2 } });
    second.resolve(applied);
    await settled();
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
  });

  it("follows a retry of the same draft to its new write, which closes the form when it lands", async () => {
    const { actor, host, submit, writes } = await started();
    const second = deferred<Answered<HttpFailure>>();
    host.answers = [Promise.resolve(refusal), second.promise];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    await settled();
    // Keep my edit: a new write of the same draft, reported to the form as taking the old one's place.
    expect(submitPatch(actor.getSnapshot().context.edits, renaming("p_2"), ABOUT)).toBeDefined();
    actor.send({ type: "resent", from: "p_1", id: "p_2" });
    await settled();
    expect(Object.keys(writes())).toEqual(["p_2"]);
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "sending", sent: { id: "p_2", version: 1 } });
    second.resolve(applied);
    await settled();
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
  });

  it("ignores a retry reported for a write the form did not send", async () => {
    const { actor, host, submit } = await started();
    host.answers = [Promise.resolve(refusal)];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    await settled();
    actor.send({ type: "resent", from: "p_other", id: "p_2" });
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "sending", sent: { id: "p_1" } });
  });
});

describe("a form restored after a reload", () => {
  const sentAt = (version: number, sentVersion: number): Opened => ({ ...opening("skip", "Waiting", SEEN), version, sent: { id: "p_1", version: sentVersion } });

  it("closes when the write it sent is no longer held and nothing was typed since", async () => {
    const { actor, last } = await started({ restored: sentAt(1, 1) });
    await settled();
    expect(formView(actor.getSnapshot())).toEqual({ state: "closed" });
    expect(last()).toBeUndefined();
  });

  it("stays editable, claiming no success, when text was typed since the write that is no longer held", async () => {
    const { actor, last } = await started({ restored: sentAt(2, 1) });
    await settled();
    expect(formView(actor.getSnapshot())).toMatchObject({ state: "editing", opened: { value: "Waiting" } });
    expect(drafted(actor).sent).toBeUndefined();
    expect(drafted(actor).kept).toBeUndefined();
    expect(last()).toMatchObject({ case: "skip", version: 2 });
  });

  it("watches again when the form is stopped and started from its saved draft", async () => {
    const { actor, begin, host, submit, saved, last } = await started();
    const answer = deferred<Answered<HttpFailure>>();
    host.answers = [answer.promise];
    actor.send({ type: "open", opened: opening("skip", "", SEEN) });
    actor.send({ type: "change", case: "skip", value: "a" });
    submit("p_1");
    actor.stop();
    const again = begin(readOpened(JSON.parse(JSON.stringify(last()))));
    expect(formView(again.getSnapshot())).toMatchObject({ state: "sending", sent: { id: "p_1" } });
    const before = saved.length;
    answer.resolve(applied);
    await settled();
    expect(formView(again.getSnapshot())).toEqual({ state: "closed" });
    // Only the machine that is running reacted to the landing.
    expect(saved.length - before).toBe(1);
  });
});
