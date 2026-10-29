// One conversation held for the tab (I5): a turn's answer carries what it added and is shown
// at once, a read begun before an answer never replaces it, an answer after the conversation
// stopped changes nothing, and the proposals of the last turn are held for review.
import { createActor } from "xstate";
import { describe, expect, it } from "vitest";

import { settled } from "../data/fake.test-support.ts";
import { conversationMachine, conversationView, sendTurn, type ConversationRef } from "./conversation.ts";
import { HandHost, answered, holding, replied, said, target } from "./hand-host.test-support.ts";

function started() {
  const host = new HandHost();
  const ref: ConversationRef = createActor(conversationMachine, { input: { assistant: host, target } }).start();
  return { host, ref };
}

/** The words the panel shows. */
function words(ref: ConversationRef): string[] {
  const { read } = conversationView(ref.getSnapshot());
  const messages = read.status === "ready" ? read.conversation.messages : [];
  return messages.flatMap((entry) => (entry.author === "cairn" ? [] : [entry.content]));
}

describe("a conversation held for the tab", () => {
  it("shows the turn as the host answered it, with no panel open and no second read", async () => {
    const { host, ref } = started();
    host.reads[0]?.answer(holding([said("Earlier.", "t0")]));
    await settled();
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer(replied("Hi."));
    expect(await sent).toBe(true);
    expect(words(ref)).toEqual(["Earlier.", "Hello?", "Hi."]);
    expect(host.reads).toHaveLength(1);
  });

  it("holds the proposals of the last turn for review, and none once another turn is sent", async () => {
    const { host, ref } = started();
    const proposals = [
      { proposal: "pr_one", destination: { journey: "j_one" } },
      { proposal: "pr_two", destination: { journey: "j_one" }, discarded: true },
    ];
    const sent = sendTurn(ref, "Draft it.");
    host.turns[0]?.answer({ outcome: "answered", reply: { conversation: "cv_one", reply: "Drafted.", proposals, ended: { status: "replied" }, messages: [] } });
    await sent;
    expect(conversationView(ref.getSnapshot()).proposals.map((each) => each.proposal)).toEqual(["pr_one"]);
    void sendTurn(ref, "Again.");
    expect(conversationView(ref.getSnapshot()).proposals).toEqual([]);
  });

  it("shows the turn in flight until it is answered", async () => {
    const { host, ref } = started();
    void sendTurn(ref, "Hello?");
    expect(conversationView(ref.getSnapshot()).sending).toBe("Hello?");
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    expect(conversationView(ref.getSnapshot()).sending).toBeUndefined();
  });

  it("shows why a turn stopped, from the answer alone", async () => {
    const { host, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    const messages = [said("Hello?", "t1"), { author: "cairn" as const, at: "t2", content: "the provider failed" }, answered("Done.", "t3")];
    host.turns[0]?.answer({ outcome: "answered", reply: { conversation: "cv_one", reply: "Done.", ended: { status: "stopped", message: "the provider failed" }, messages } });
    expect(await sent).toBe(true);
    expect(words(ref)).toEqual(["Hello?", "Done."]);
    const { read } = conversationView(ref.getSnapshot());
    expect(read.status === "ready" && read.conversation.messages.map((each) => each.author)).toEqual(["user", "cairn", "assistant"]);
  });
});

describe("a read and a turn in a conversation", () => {
  it("never lets a read begun before an answer replace it", async () => {
    const { host, ref } = started();
    ref.send({ type: "refresh" });
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer(replied("Hi."));
    await sent;
    // Every read begun before the answer answers last, with a conversation that lacks the turn.
    for (const read of host.reads) {
      read.answer(holding([]));
    }
    await settled();
    expect(words(ref)).toEqual(["Hello?", "Hi."]);
  });

  it("starts no read while a turn is out, and a read after it", async () => {
    const { host, ref } = started();
    const asked = host.reads.length;
    const sent = sendTurn(ref, "Hello?");
    ref.send({ type: "refresh" });
    expect(host.reads).toHaveLength(asked);
    host.turns[0]?.answer(replied("Hi."));
    await sent;
    ref.send({ type: "refresh" });
    expect(host.reads).toHaveLength(asked + 1);
    host.reads.at(-1)?.answer(holding([said("Hello?", "t1"), answered("Hi.", "t2")]));
    await settled();

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
    const { host, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    ref.stop();
    host.turns[0]?.answer(replied("Hi."));
    await settled();
    expect(await sent).toBe(false);
    expect(host.reads).toHaveLength(1);
    expect(words(ref)).toEqual([]);
  });
});

describe("a turn sent from a panel", () => {
  it("sends one turn for a double send before anything re-renders", async () => {
    const { host, ref } = started();
    const first = sendTurn(ref, "Hello?");
    const second = sendTurn(ref, "Hello?");
    expect(await second).toBe(false);
    expect(host.turns).toHaveLength(1);
    host.turns[0]?.answer(replied("Hi."));
    expect(await first).toBe(true);
  });

  it("answers false for a turn the host did not answer, and shows why", async () => {
    const { host, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer({ outcome: "failed", error: { status: 503, message: "busy" } });
    expect(await sent).toBe(false);
    expect(conversationView(ref.getSnapshot()).failure).toEqual({ status: 503, message: "busy" });
    expect(conversationView(ref.getSnapshot()).sending).toBeUndefined();
  });

  it.each([0, 502, 504])("says a turn that failed with status %i may have written", async (status) => {
    const { host, ref } = started();
    const sent = sendTurn(ref, "Hello?");
    host.turns[0]?.answer({ outcome: "failed", error: { status, message: "gateway" } });
    expect(await sent).toBe(false);
    expect(conversationView(ref.getSnapshot()).failure?.message).toContain("check the journey");
  });
});

describe("the conversation a panel shows", () => {
  it("shows a failed read as it is", async () => {
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
