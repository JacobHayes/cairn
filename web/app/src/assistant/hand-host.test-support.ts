// An assistant whose reads and turns the test answers by hand, in the order it likes.
import type { AssistantHost, AssistantTarget, Conversation, ConversationEntry, TurnOutcome } from "../data/assistant.ts";
import { targetKey } from "../data/assistant.ts";

export const target: AssistantTarget = { journey: "j_one" };

export interface PendingRead {
  target: AssistantTarget;
  answer(conversation: Conversation): void;
  fail(thrown: unknown): void;
}

export interface PendingTurn {
  target: AssistantTarget;
  message: string;
  answer(outcome: TurnOutcome): void;
}

export class HandHost implements AssistantHost {
  /** Every read asked, in order, answered or not. */
  readonly reads: PendingRead[] = [];
  /** Every turn sent, in order. */
  readonly turns: PendingTurn[] = [];

  conversation(of: AssistantTarget): Promise<Conversation> {
    return new Promise((resolve, reject) => {
      this.reads.push({ target: of, answer: resolve, fail: reject });
    });
  }

  turn(of: AssistantTarget, message: string): Promise<TurnOutcome> {
    return new Promise((resolve) => {
      this.turns.push({ target: of, message, answer: resolve });
    });
  }

  /** How many reads of `of` were asked. */
  readsOf(of: AssistantTarget): number {
    return this.reads.filter((each) => targetKey(each.target) === targetKey(of)).length;
  }
}

/** The conversation `cv_one` holding `messages`. */
export const holding = (messages: ConversationEntry[]): Conversation => ({ conversation: "cv_one", messages });

/** A turn answered with `reply`. */
export const replied = (reply: string): TurnOutcome => ({ outcome: "answered", reply: { conversation: "cv_one", reply, ended: { status: "replied" } } });

export const said = (content: string, at: string): ConversationEntry => ({ author: "user", at, content });
export const answered = (content: string, at: string): ConversationEntry => ({ author: "assistant", at, content });
