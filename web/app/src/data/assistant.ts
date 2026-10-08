// The assistant over the server host (I5; ARCHITECTURE, Assistant): one turn of the caller's
// conversation about a journey or a route's draft (`POST /api/journeys/{id}/assistant`,
// `POST /api/routes/{id}/draft/assistant`), and that conversation read back as kept (`GET` on the
// same paths). The in-browser host has no assistant (ARCHITECTURE, Service layer and
// composition: its root assembles none), so it has no `AssistantHost` at all.
import { answerFailure, networkFailure, type CairnClient, type HttpFailure, type Schema } from "@cairn/client";

export type TurnReply = Schema<"TurnReply">;
export type AssistantAction = Schema<"Action">;
export type Conversation = Schema<"Conversation">;
export type ConversationEntry = Schema<"ConversationEntry">;
export type Ended = Schema<"Ended">;
export type Because = Schema<"Because">;

/** What a conversation is about: a journey, or a route's draft (which may not be open yet). */
export type AssistantTarget = { journey: string } | { route: string };

/** How a turn ended: the assistant's answer, or why there is none (busy, refused, failed). */
export type TurnOutcome = { outcome: "answered"; reply: TurnReply } | { outcome: "failed"; error: HttpFailure };

/** The assistant of a host that offers one. */
export interface AssistantHost {
  /** I5: the caller's conversation about `target`, as kept; empty before its first turn. */
  conversation(target: AssistantTarget): Promise<Conversation>;
  /** I5: one turn: `message` sent, answered once the assistant has finished. */
  turn(target: AssistantTarget, message: string): Promise<TurnOutcome>;
}

/** The key a target's conversation is kept under in the tab: one per target. */
export function targetKey(target: AssistantTarget): string {
  return "journey" in target ? `journey:${target.journey}` : `route:${target.route}`;
}

interface Reply<T> {
  data?: T;
  error?: unknown;
  response: Response;
}

async function answered<T>(call: () => Promise<Reply<T>>): Promise<{ data: T } | { error: HttpFailure }> {
  let reply: Reply<T>;
  try {
    reply = await call();
  } catch (thrown) {
    return { error: networkFailure(thrown) };
  }
  return reply.data === undefined ? { error: answerFailure(reply.response.status, reply.error) } : { data: reply.data };
}

/** The server's assistant, through `client`. */
export function serverAssistant(client: CairnClient): AssistantHost {
  return {
    conversation: async (target) => {
      const read = await answered(() =>
        "journey" in target
          ? client.GET("/api/journeys/{id}/assistant", { params: { path: { id: target.journey } } })
          : client.GET("/api/routes/{id}/draft/assistant", { params: { path: { id: target.route } } }),
      );
      if ("error" in read) {
        throw new Error(read.error.message);
      }
      return read.data;
    },
    turn: async (target, message) => {
      const body = { message };
      const sent = await answered(() =>
        "journey" in target
          ? client.POST("/api/journeys/{id}/assistant", { params: { path: { id: target.journey } }, body })
          : client.POST("/api/routes/{id}/draft/assistant", { params: { path: { id: target.route } }, body }),
      );
      return "error" in sent ? { outcome: "failed", error: sent.error } : { outcome: "answered", reply: sent.data };
    },
  };
}
