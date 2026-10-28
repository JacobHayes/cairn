// One conversation held for the tab (I5): a turn's answer is kept whether or not a panel is
// open, a read begun before an answer never replaces it, an answer after the conversation
// stopped changes nothing, and a turn the host answered but did not keep stays in its place
// among later turns.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { DRAFT_PREFIX } from "../data/drafts.ts";
import { memoryStorage, memoryTurns, settled } from "../data/fake.test-support.ts";
import { conversationMachine, conversationView, forgetEarlierKeptTurns, keptKey, sendTurn, type ConversationRef } from "./conversation.ts";
import { HandHost, answered, holding, replied, said, target } from "./hand-host.test-support.ts";
import { shown } from "./model.ts";

function started() {
  const host = new HandHost();
  const turns = memoryTurns();
  const ref: ConversationRef = createActor(conversationMachine, { input: { assistant: host, target, turns } }).start();
  return { host, turns, ref };
}

/** The words the panel shows. */
function words(ref: ConversationRef): string[] {
  const { read, kept, unsaved } = conversationView(ref.getSnapshot());
  const messages = read.status === "ready" ? read.conversation.messages : [];
  return shown(messages, kept, unsaved).flatMap((entry) => (entry.entry === "said" || entry.entry === "reply" ? [entry.text] : []));
}

describe("a conversation held for the tab", () => {
  it("keeps an answer the host did not keep, with no panel open", async () => {
    const { host, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    host.reads[1]?.answer(holding([]));
    expect(await sent).toBe(true);
    expect(words(ref)).toEqual(["Hello?", "Hi."]);
  });

  it("shows the turn in flight until the conversation has been read again", async () => {
    const { host, ref } = started();
    void sendTurn(ref, "Hello?");
    expect(conversationView(ref.getSnapshot()).sending).toBe("Hello?");
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    expect(conversationView(ref.getSnapshot()).sending).toBe("Hello?");
    host.reads[1]?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    await settled();
    expect(conversationView(ref.getSnapshot()).sending).toBeUndefined();
  });

});

describe("a read and a turn in a conversation", () => {
  it("never lets a read begun before an answer replace it", async () => {
    const { host, ref } = started();
    ref.send({ type: "refresh" });
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    host.reads.at(-1)?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    await sent;
    // Every read begun before the answer answers last, with a conversation that lacks the turn.
    for (const read of host.reads.slice(0, -1)) {
      read.answer(holding([]));
    }
    await settled();
    expect(words(ref)).toEqual(["Hello?", "Hi."]);
  });

  it("starts no read while a turn is out, and one when it ends: the re-read, or after a failed turn a read", async () => {
    const { host, ref } = started();
    const asked = host.reads.length;
    const sent = sendTurn(ref, "Hello?");
    ref.send({ type: "refresh" });
    expect(host.reads).toHaveLength(asked);
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    ref.send({ type: "refresh" });
    expect(host.reads).toHaveLength(asked + 1);
    host.reads.at(-1)?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    await sent;

    const failed = sendTurn(ref, "Again?");
    ref.send({ type: "refresh" });
    expect(host.reads).toHaveLength(asked + 1);
    host.turns[1]?.answer({ outcome: "failed", error: { status: 503, message: "busy" } });
    expect(await failed).toBe(false);
    await settled();
    // A panel reopened during the failed turn still gets its fresh read, and the failure stays shown after it.
    expect(host.reads).toHaveLength(asked + 2);
    host.reads.at(-1)?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    await settled();
    expect(conversationView(ref.getSnapshot()).failure).toMatchObject({ status: 503 });
    expect(ref.getSnapshot().hasTag("busy")).toBe(false);
  });

  it("changes nothing when its turn is answered after the conversation stopped", async () => {
    const { host, turns, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    ref.stop();
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    expect(await sent).toBe(false);
    expect(host.reads).toHaveLength(1);
    expect(words(ref)).toEqual([]);
    expect(turns.read("cv_one")).toEqual([]);
  });

  it("writes no kept turn when it stopped while reading the conversation again", async () => {
    const { host, turns, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    ref.stop();
    host.reads.at(-1)?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    await settled();
    expect(await sent).toBe(false);
    expect(turns.read("cv_one")).toEqual([]);
  });

});

describe("a turn sent from a panel", () => {
  it("sends one turn for a double send before anything re-renders", async () => {
    const { host, ref } = started();
    const first = sendTurn(ref, "Hello?");
    const second = sendTurn(ref, "Hello?");
    expect(await second).toBe(false);
    expect(host.turns).toHaveLength(1);
    let settledFirst = false;
    void first.then(() => {
      settledFirst = true;
    });
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    // The first answers only once the conversation has been read again.
    expect(settledFirst).toBe(false);
    host.reads.at(-1)?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    expect(await first).toBe(true);
  });

  it("answers true for a turn whose re-read failed, keeps its words, and accepts a later turn", async () => {
    const { host, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    host.reads.at(-1)?.fail(new Error("down"));
    expect(await sent).toBe(true);
    expect(words(ref)).toEqual(["Hello?", "Hi."]);
    expect(conversationView(ref.getSnapshot()).failure?.message).toContain("down");
    const next = sendTurn(ref, "Still there?");
    expect(host.turns).toHaveLength(2);
    host.turns[1]?.answer(replied("Yes."));
    await settled();
    host.reads.at(-1)?.answer(holding([]));
    expect(await next).toBe(true);
    expect(words(ref)).toEqual(["Hello?", "Hi.", "Still there?", "Yes."]);
  });

  it("answers false for a turn the host did not answer, and shows why", async () => {
    const { host, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer({ outcome: "failed", error: { status: 503, message: "busy" } });
    expect(await sent).toBe(false);
    expect(conversationView(ref.getSnapshot()).failure).toEqual({ status: 503, message: "busy" });
    expect(conversationView(ref.getSnapshot()).sending).toBeUndefined();
  });

});

describe("the turns a conversation keeps", () => {
  it("shows a turn the host did not keep before the turns kept after it", async () => {
    const { host, ref } = started();
    host.reads[0]?.answer(holding([said("First.", "t1"), answered("One.", "t2")]));
    await settled();
    const unkept = sendTurn(ref, "Second.");
    host.turns[0]?.answer(replied("Two."));
    await settled();
    host.reads.at(-1)?.answer(holding([said("First.", "t1"), answered("One.", "t2")]));
    await unkept;
    const kept = sendTurn(ref, "Third.");
    host.turns[1]?.answer(replied("Three."));
    await settled();
    host.reads.at(-1)?.answer(holding([said("First.", "t1"), answered("One.", "t2"), said("Third.", "t5"), answered("Three.", "t6")]));
    await kept;
    expect(words(ref)).toEqual(["First.", "One.", "Second.", "Two.", "Third.", "Three."]);
  });

  it("keeps the conversation it holds while a refresh reads it again", async () => {
    const { host, ref } = started();
    host.reads[0]?.answer(holding([said("First.", "t1")]));
    await settled();
    ref.send({ type: "refresh" });
    expect(words(ref)).toEqual(["First."]);
    host.reads[1]?.fail(new Error("down"));
    await settled();
    expect(conversationView(ref.getSnapshot()).read).toEqual({ status: "failed", message: "down" });
  });
});

describe("turns kept by an earlier build", () => {
  it("are removed at boot, and this build's turns stay", () => {
    const storage = memoryStorage();
    // A proposed action kept before proposals named their destination, and a direct change
    // kept before it said what it did: this build cannot show either.
    const proposed = [{ at: "t2", actions: [{ outcome: "proposed", tool: "create_proposal", proposal: "p_1", because: { reason: "asked" } }], ended: { status: "replied" } }];
    const applied = [{ at: "t3", actions: [{ outcome: "applied", tool: "transition_node", receipt: { patch_id: "p_2", domain: { journey: "j_one" }, revision: 2 } }], ended: { status: "replied" } }];
    storage.setItem(`${DRAFT_PREFIX}assistant-turns:c_1`, JSON.stringify(proposed));
    storage.setItem(`${DRAFT_PREFIX}assistant-turns-2:c_1`, JSON.stringify(applied));
    storage.setItem(`${DRAFT_PREFIX}${keptKey("c_1")}`, JSON.stringify([]));
    storage.setItem(`${DRAFT_PREFIX}title:j_one:n_a`, JSON.stringify("Unrelated"));
    forgetEarlierKeptTurns(storage);
    const keys = Array.from({ length: storage.length }, (_, at) => storage.key(at)).sort();
    expect(keys).toEqual([`${DRAFT_PREFIX}${keptKey("c_1")}`, `${DRAFT_PREFIX}title:j_one:n_a`].sort());
  });
});
