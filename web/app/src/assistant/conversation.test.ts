// One conversation held for the tab (I5): a turn's answer is kept whether or not a panel is
// open, a read begun before an answer never replaces it, and a turn the host answered but
// did not keep stays in its place among later turns.
import { describe, expect, it } from "vitest";

import type { AssistantHost, AssistantTarget, Conversation, ConversationEntry, TurnOutcome } from "../data/assistant.ts";
import { settled } from "../data/fake.test-support.ts";
import { AssistantConversation } from "./conversation.ts";
import { shown } from "./model.ts";

/** A host whose reads and turns the test answers by hand, in order. */
class HandHost implements AssistantHost {
  readonly reads: ((conversation: Conversation) => void)[] = [];
  readonly turns: ((outcome: TurnOutcome) => void)[] = [];

  conversation(): Promise<Conversation> {
    return new Promise((resolve) => this.reads.push(resolve));
  }

  turn(): Promise<TurnOutcome> {
    return new Promise((resolve) => this.turns.push(resolve));
  }

  /** Answers the oldest unanswered read with `messages`. */
  answerRead(messages: ConversationEntry[]): void {
    this.reads.shift()?.({ conversation: "cv_one", messages });
  }

  answerTurn(reply: string): void {
    this.turns.shift()?.({ outcome: "answered", reply: { conversation: "cv_one", reply, ended: { status: "replied" } } });
  }
}

const target: AssistantTarget = { journey: "j_one" };
const said = (content: string, at: string): ConversationEntry => ({ author: "user", at, content });
const replied = (content: string, at: string): ConversationEntry => ({ author: "assistant", at, content });

function words(conversation: AssistantConversation): string[] {
  const { read, kept, unsaved } = conversation.view;
  const messages = read.status === "ready" ? read.conversation.messages : [];
  return shown(messages, kept, unsaved).flatMap((entry) => (entry.entry === "said" || entry.entry === "reply" ? [entry.text] : []));
}

describe("a conversation held for the tab", () => {
  it("keeps an answer the host did not keep, with no panel open", async () => {
    const host = new HandHost();
    const conversation = new AssistantConversation(host, target);
    const sent = conversation.send("Hello?");
    host.answerTurn("Hi.");
    await settled();
    host.answerRead([]);
    expect(await sent).toBe(true);
    expect(words(conversation)).toEqual(["Hello?", "Hi."]);
  });

  it("never lets a read begun before an answer replace it", async () => {
    const host = new HandHost();
    const conversation = new AssistantConversation(host, target);
    const sent = conversation.send("Hello?");
    conversation.refresh();
    host.answerTurn("Hi.");
    await settled();
    // The read after the turn answers first; the opening read, begun before, answers last.
    host.reads.reverse();
    host.answerRead([said("Hello?", "t1"), replied("Hi.", "t2")]);
    await sent;
    host.answerRead([]);
    await settled();
    expect(words(conversation)).toEqual(["Hello?", "Hi."]);
  });

  it("shows a turn the host did not keep before the turns kept after it", async () => {
    const host = new HandHost();
    const conversation = new AssistantConversation(host, target);
    conversation.refresh();
    host.answerRead([said("First.", "t1"), replied("One.", "t2")]);
    await settled();
    const unkept = conversation.send("Second.");
    host.answerTurn("Two.");
    await settled();
    host.answerRead([said("First.", "t1"), replied("One.", "t2")]);
    await unkept;
    const kept = conversation.send("Third.");
    host.answerTurn("Three.");
    await settled();
    host.answerRead([said("First.", "t1"), replied("One.", "t2"), said("Third.", "t5"), replied("Three.", "t6")]);
    await kept;
    expect(words(conversation)).toEqual(["First.", "One.", "Second.", "Two.", "Third.", "Three."]);
  });
});
