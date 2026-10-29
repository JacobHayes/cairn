// The assistant panel's model (I5, I7): what it says it is about. The conversation itself is
// the host's: the user's words, Cairn's receipt line for each write, and the replies, shown as
// kept.
import type { AssistantTarget } from "../data/assistant.ts";

/** What the panel says the conversation is about. */
export function aboutWords(target: AssistantTarget): string {
  return "journey" in target ? "this journey" : "this route's draft";
}
