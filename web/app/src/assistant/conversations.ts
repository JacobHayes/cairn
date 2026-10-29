// The session's conversation manager (I5): one conversation per assistant target, in an
// explicit keyed map, started when a panel first opens it and kept for the tab. A conversation
// outlives the panel, so a turn answered while the panel is closed or showing another target
// still lands in its own conversation. The root stops them all when it stops.
import { enqueueActions, setup, type ActorRefFrom } from "xstate";

import { targetKey, type AssistantHost, type AssistantTarget } from "../data/assistant.ts";
import { conversationMachine, type ConversationRef } from "./conversation.ts";

export interface ConversationsInput {
  /** The host's assistant. */
  assistant: AssistantHost;
}

interface ConversationsContext extends ConversationsInput {
  conversations: Readonly<Record<string, ConversationRef>>;
}

export type ConversationsEvent =
  /** A panel opened `target`: its conversation is started, or read again if the tab holds it. */
  { type: "open"; target: AssistantTarget };

export const conversationManager = setup({
  types: { context: {} as ConversationsContext, events: {} as ConversationsEvent, input: {} as ConversationsInput },
  actors: { conversation: conversationMachine },
  actions: {
    open: enqueueActions(({ context, event, enqueue }) => {
      const key = targetKey(event.target);
      const held = context.conversations[key];
      const { assistant } = context;
      if (held !== undefined) {
        enqueue.sendTo(held, { type: "refresh" });
      } else {
        // A new conversation starts by reading.
        enqueue.assign(({ context: current, spawn }) => ({
          conversations: { ...current.conversations, [key]: spawn("conversation", { id: key, input: { assistant, target: event.target } }) },
        }));
      }
    }),
  },
}).createMachine({
  id: "conversations",
  context: ({ input }) => ({ ...input, conversations: {} }),
  on: { open: { actions: "open" } },
});

export type ConversationsRef = ActorRefFrom<typeof conversationManager>;
