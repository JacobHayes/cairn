// The assistant panel's model (I5, I7): whether the panel is offered, and what it says it is
// about. The conversation itself is the host's: the user's words, Cairn's receipt line for each
// write, and the replies, shown as kept.
import type { Session } from "../data/session.ts";
import type { AssistantTarget } from "../data/assistant.ts";

/**
 * Capabilities gating (ARCHITECTURE, Terms: Capabilities document): the panel is offered only
 * when the host's capabilities say it has an assistant and the host can reach one. The
 * cairn demo has neither, so it never shows the panel (I5: fully usable without it).
 */
export function assistantOffered(session: Pick<Session, "capabilities" | "host">): boolean {
  return session.capabilities.assistant && session.host.assistant !== undefined;
}

/** What the panel says the conversation is about. */
export function aboutWords(target: AssistantTarget): string {
  return "journey" in target ? "this journey" : "this route's draft";
}
