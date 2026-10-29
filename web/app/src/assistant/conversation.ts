// One conversation as the panel shows it (I5), held per target for the tab by the session's
// conversation manager rather than by a mounted panel: what the host keeps, a turn in flight,
// and the proposals the last turn drafted. A panel closed while a turn runs shows its answer
// when opened again, and a read begun before an answer arrived never replaces the answer.
//
// The read and the turn are invoked actors, and the guarantee holds by construction rather
// than by counting: a turn stops the read in flight by leaving `reading`, and nothing starts
// another read while it is out. A turn's answer carries what it added to the conversation,
// shown as it arrives and replaced by the host's own on the next read. An answer that arrives
// after the conversation stopped (the session closed) changes nothing, because a stopped
// actor hears nothing.
import { networkFailure, type HttpFailure } from "@cairn/client";
import { assign, fromPromise, setup, waitFor, type ActorRefFrom, type SnapshotFrom } from "xstate";

import type { AssistantHost, AssistantTarget, Conversation, Proposed, TurnOutcome } from "../data/assistant.ts";

export type Read = { status: "loading" } | { status: "failed"; message: string } | { status: "ready"; conversation: Conversation };

/** What the panel shows of one conversation. */
export interface ConversationView {
  read: Read;
  /** The proposals the last turn left for review. */
  proposals: readonly Proposed[];
  /** The words of the turn in flight, if one is. */
  sending: string | undefined;
  /** Why the last turn was not answered, or its conversation not read again. */
  failure: HttpFailure | undefined;
}

export interface ConversationInput {
  assistant: AssistantHost;
  target: AssistantTarget;
}

interface ConversationContext extends ConversationInput {
  read: Read;
  proposals: readonly Proposed[];
  /** The words of the turn sent last. */
  text: string;
  failure: HttpFailure | undefined;
}

/** What the failure says of a turn whose answer never arrived: it may have written. */
export const ANSWER_LOST = "The reply did not arrive. The assistant may have made changes: check the journey before sending again.";

export type ConversationEvent =
  /** Reads the conversation again, as the panel opens. */
  | { type: "refresh" }
  /** I5: sends `text` as one turn. */
  | { type: "send"; text: string };

const message = (thrown: unknown): string => (thrown instanceof Error ? thrown.message : String(thrown));

/** A failure of a turn: when no answer came back (the connection, or a gateway in between, failed), the host may still have acted on it. */
const unanswered = (failure: HttpFailure): HttpFailure => ([0, 502, 504].includes(failure.status) ? { status: 0, message: ANSWER_LOST } : failure);

const answered = (outcome: TurnOutcome): outcome is Extract<TurnOutcome, { outcome: "answered" }> => outcome.outcome === "answered";

export const conversationMachine = setup({
  types: { context: {} as ConversationContext, events: {} as ConversationEvent, input: {} as ConversationInput },
  actors: {
    read: fromPromise<Conversation, ConversationInput>(({ input }) => input.assistant.conversation(input.target)),
    turn: fromPromise<TurnOutcome, ConversationInput & { text: string }>(({ input }) => input.assistant.turn(input.target, input.text)),
  },
  actions: {
    begin: assign({ text: ({ event }) => (event as Extract<ConversationEvent, { type: "send" }>).text, failure: undefined, proposals: [] }),
    ready: assign({ read: ({ event }) => ({ status: "ready" as const, conversation: (event as unknown as { output: Conversation }).output }) }),
  },
}).createMachine({
  id: "conversation",
  context: ({ input }) => ({ ...input, read: { status: "loading" }, proposals: [], text: "", failure: undefined }),
  initial: "reading",
  states: {
    idle: { on: { refresh: "reading", send: { target: "sending", actions: "begin" } } },
    // Entering this state never resets what is held: a refresh keeps showing the conversation.
    reading: {
      invoke: {
        src: "read",
        input: ({ context }) => ({ assistant: context.assistant, target: context.target }),
        onDone: { target: "idle", actions: "ready" },
        onError: { target: "idle", actions: assign({ read: ({ event }) => ({ status: "failed" as const, message: message(event.error) }) }) },
      },
      on: { refresh: { target: "reading", reenter: true }, send: { target: "sending", actions: "begin" } },
    },
    // A turn: not a refresh, not another send. A read in flight when it starts is stopped by leaving `reading`.
    sending: {
      tags: ["busy"],
      invoke: {
        src: "turn",
        input: ({ context }) => ({ assistant: context.assistant, target: context.target, text: context.text }),
        onDone: [
          { guard: ({ event }) => !answered(event.output), target: "reading", actions: assign({ failure: ({ event }) => (event.output.outcome === "failed" ? unanswered(event.output.error) : undefined) }) },
          {
            target: "idle",
            actions: assign(({ context, event }) => {
              const reply = answered(event.output) ? event.output.reply : undefined;
              const held = context.read.status === "ready" ? context.read.conversation.messages : [];
              return {
                read: reply === undefined ? context.read : { status: "ready" as const, conversation: { conversation: reply.conversation, messages: [...held, ...(reply.messages)] } },
                proposals: (reply?.proposals ?? []).filter((each) => each.discarded !== true),
              };
            }),
          },
        ],
        // The request may have reached the host: what it wrote is in the conversation, read again.
        onError: { target: "reading", actions: assign({ failure: ({ event }) => unanswered(networkFailure(event.error)) }) },
      },
    },
  },
});

export type ConversationRef = ActorRefFrom<typeof conversationMachine>;
export type ConversationSnapshot = SnapshotFrom<typeof conversationMachine>;

/** What the panel shows of the conversation: the turn in flight is the state, not a field beside it. */
export function conversationView(snapshot: ConversationSnapshot): ConversationView {
  const { read, proposals, text, failure } = snapshot.context;
  return { read, proposals, sending: snapshot.hasTag("busy") ? text : undefined, failure };
}

/**
 * I5: sends `text` as one turn; answers whether the host answered it. False for a turn the
 * host did not answer, one sent while another is out, and one whose conversation stopped
 * meanwhile.
 */
export async function sendTurn(ref: ConversationRef, text: string): Promise<boolean> {
  if (ref.getSnapshot().hasTag("busy")) {
    return false;
  }
  ref.send({ type: "send", text });
  if (!ref.getSnapshot().matches("sending")) {
    return false;
  }
  try {
    const left = await waitFor(ref, (snapshot) => !snapshot.matches("sending"), { timeout: Infinity });
    return left.matches("idle");
  } catch {
    return false;
  }
}
