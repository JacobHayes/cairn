// One conversation as the panel shows it (I5), held per target for the tab by the session's
// conversation manager rather than by a mounted panel: what the host keeps, the tab's kept
// turns beside it, a turn in flight, and any turn the host answered but did not keep. A panel
// closed while a turn runs shows its answer when opened again, and a read begun before an
// answer arrived never replaces the answer.
//
// The read and the turn are invoked actors, and the guarantee holds by construction rather
// than by counting: a turn stops the read in flight by leaving `reading`, it ends in a read of
// its own (`rereading`, or `reading` when the turn failed), and nothing starts another read
// while it is out. An answer that arrives after the conversation stopped (the session
// closed) changes nothing, because a stopped actor hears nothing.
import { networkFailure, type HttpFailure } from "@cairn/client";
import { assign, fromPromise, setup, waitFor, type ActorRefFrom, type SnapshotFrom } from "xstate";

import type { AssistantHost, AssistantTarget, Conversation, TurnOutcome, TurnReply } from "../data/assistant.ts";
import { DRAFT_PREFIX, readDraft, writeDraft } from "../data/drafts.ts";
import { answeredEntries, holdsTurn, keeping, keptTurn, latestAnswerAt, type KeptTurn, type Unsaved } from "./model.ts";

export type Read = { status: "loading" } | { status: "failed"; message: string } | { status: "ready"; conversation: Conversation };

/** What the panel shows of one conversation. */
export interface ConversationView {
  read: Read;
  kept: readonly KeptTurn[];
  unsaved: readonly Unsaved[];
  /** The words of the turn in flight, if one is. */
  sending: string | undefined;
  /** Why the last turn was not answered, or its conversation not read again. */
  failure: HttpFailure | undefined;
}

/** The tab's kept turns of each conversation (session storage, beside the drafts). */
export interface KeptTurns {
  read(conversation: string): KeptTurn[];
  write(conversation: string, turns: KeptTurn[]): void;
}

/**
 * Where a conversation's kept turns are stored. The version names the turns' shape: a tab open
 * across a deploy that changed it holds turns under an earlier version's key, which this build
 * never reads (a proposed action kept before proposals named their destination has no address).
 */
export const keptKey = (conversation: string): string => `assistant-turns-2:${conversation}`;

/** What every earlier build's kept-turn keys start with. */
const EARLIER_KEPT = `${DRAFT_PREFIX}assistant-turns:`;

/**
 * Removes the turns an earlier build kept in `storage` (a tab's session storage; none where it
 * is unavailable), once at boot. Unread, they would only take space: without them the
 * conversation shows those turns' reports as Cairn kept them, as text. Goes once one release has
 * shipped it.
 */
export function forgetEarlierKeptTurns(storage: Storage | undefined): void {
  if (storage === undefined) {
    return;
  }
  try {
    const earlier = Array.from({ length: storage.length }, (_, at) => storage.key(at)).filter((key): key is string => key?.startsWith(EARLIER_KEPT) === true);
    for (const key of earlier) {
      storage.removeItem(key);
    }
  } catch {
    // Storage refused: the earlier keys stay, unread.
  }
}

export const draftTurns: KeptTurns = {
  read: (conversation) => readDraft<KeptTurn[]>(keptKey(conversation)) ?? [],
  write: (conversation, turns) => {
    writeDraft(keptKey(conversation), turns);
  },
};

export interface ConversationInput {
  assistant: AssistantHost;
  target: AssistantTarget;
  turns: KeptTurns;
}

interface ConversationContext extends ConversationInput {
  read: Read;
  kept: readonly KeptTurn[];
  unsaved: readonly Unsaved[];
  /** The words of the turn sent last. */
  text: string;
  /** What the host answered it, until the conversation has been read again. */
  reply: TurnReply | undefined;
  failure: HttpFailure | undefined;
}

export type ConversationEvent =
  /** Reads the conversation again, as the panel opens. */
  | { type: "refresh" }
  /** I5: sends `text` as one turn. */
  | { type: "send"; text: string };

const message = (thrown: unknown): string => (thrown instanceof Error ? thrown.message : String(thrown));

const answered = (outcome: TurnOutcome): outcome is Extract<TurnOutcome, { outcome: "answered" }> => outcome.outcome === "answered";

export const conversationMachine = setup({
  types: { context: {} as ConversationContext, events: {} as ConversationEvent, input: {} as ConversationInput },
  actors: {
    read: fromPromise<Conversation, ConversationInput>(({ input }) => input.assistant.conversation(input.target)),
    turn: fromPromise<TurnOutcome, ConversationInput & { text: string }>(({ input }) => input.assistant.turn(input.target, input.text)),
  },
  actions: {
    begin: assign({ text: ({ event }) => (event as Extract<ConversationEvent, { type: "send" }>).text, failure: undefined, reply: undefined }),
    // The turn's answer is kept beside the conversation when the host holds it too.
    keep: ({ context }) => {
      if (context.read.status === "ready") {
        context.turns.write(context.read.conversation.conversation, [...context.kept]);
      }
    },
  },
}).createMachine({
  id: "conversation",
  context: ({ input }) => ({ ...input, read: { status: "loading" }, kept: [], unsaved: [], text: "", reply: undefined, failure: undefined }),
  initial: "reading",
  states: {
    idle: { on: { refresh: "reading", send: { target: "sending", actions: "begin" } } },
    // Entering this state never resets what is held: a refresh keeps showing the conversation.
    reading: {
      invoke: {
        src: "read",
        input: ({ context }) => ({ assistant: context.assistant, target: context.target, turns: context.turns }),
        onDone: {
          target: "idle",
          actions: assign(({ context, event }) => ({
            read: { status: "ready" as const, conversation: event.output },
            kept: context.turns.read(event.output.conversation),
          })),
        },
        onError: { target: "idle", actions: assign({ read: ({ event }) => ({ status: "failed" as const, message: message(event.error) }) }) },
      },
      on: { refresh: { target: "reading", reenter: true }, send: { target: "sending", actions: "begin" } },
    },
    // A turn: not a refresh, not another send. A read in flight when it starts is stopped by leaving `reading`.
    sending: {
      tags: ["busy"],
      invoke: {
        src: "turn",
        input: ({ context }) => ({ assistant: context.assistant, target: context.target, turns: context.turns, text: context.text }),
        onDone: [
          { guard: ({ event }) => !answered(event.output), target: "reading", actions: assign({ failure: ({ event }) => (event.output.outcome === "failed" ? event.output.error : undefined) }) },
          { target: "rereading", actions: assign({ reply: ({ event }) => (answered(event.output) ? event.output.reply : undefined) }) },
        ],
        onError: { target: "reading", actions: assign({ failure: ({ event }) => networkFailure(event.error) }) },
      },
    },
    // The turn was answered: the conversation is read again, and the turn is kept or held out of it.
    rereading: {
      tags: ["busy"],
      invoke: {
        src: "read",
        input: ({ context }) => ({ assistant: context.assistant, target: context.target, turns: context.turns }),
        onDone: [
          {
            guard: ({ context, event }) => context.reply !== undefined && holdsTurn(event.output.messages, context.text, context.reply),
            target: "idle",
            actions: [
              assign(({ context, event }) => {
                const stored = context.turns.read(event.output.conversation);
                const at = latestAnswerAt(event.output.messages);
                const reply = context.reply;
                return {
                  read: { status: "ready" as const, conversation: event.output },
                  kept: at === undefined || reply === undefined ? stored : keeping(stored, keptTurn(at, reply)),
                  reply: undefined,
                };
              }),
              "keep",
            ],
          },
          {
            target: "idle",
            actions: assign(({ context, event }) => ({
              read: { status: "ready" as const, conversation: event.output },
              unsaved: context.reply === undefined ? context.unsaved : [...context.unsaved, { after: event.output.messages.length, entries: answeredEntries(context.text, context.reply, new Date().toISOString()) }],
              reply: undefined,
            })),
          },
        ],
        onError: {
          target: "idle",
          actions: assign(({ context, event }) => ({
            unsaved:
              context.reply === undefined
                ? context.unsaved
                : [...context.unsaved, { after: context.read.status === "ready" ? context.read.conversation.messages.length : 0, entries: answeredEntries(context.text, context.reply, new Date().toISOString()) }],
            failure: { status: 0, message: `the reply arrived, but the conversation could not be read again (${message(event.error)})` },
            reply: undefined,
          })),
        },
      },
    },
  },
});

export type ConversationRef = ActorRefFrom<typeof conversationMachine>;
export type ConversationSnapshot = SnapshotFrom<typeof conversationMachine>;

/** What the panel shows of the conversation: the turn in flight is the state, not a field beside it. */
export function conversationView(snapshot: ConversationSnapshot): ConversationView {
  const { read, kept, unsaved, text, failure } = snapshot.context;
  return { read, kept, unsaved, sending: snapshot.hasTag("busy") ? text : undefined, failure };
}

/**
 * I5: sends `text` as one turn; answers whether the host answered it, once the conversation
 * has been read again (also when that read failed). False for a turn the host did not answer,
 * one sent while another is out, and one whose conversation stopped meanwhile.
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
    if (!left.matches("rereading")) {
      return false;
    }
    await waitFor(ref, (snapshot) => !snapshot.hasTag("busy"), { timeout: Infinity });
    return true;
  } catch {
    return false;
  }
}
