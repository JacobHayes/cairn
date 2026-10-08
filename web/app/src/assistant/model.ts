// The assistant panel's model (I5, I7): whether the panel is offered, and the conversation as
// the panel shows it. The host keeps the conversation (one per target per user) with Cairn's
// report of each write as text; a turn sent from this tab also answers each write's structure
// (the nodes a direct change wrote, a proposal's id), which the tab keeps beside the
// conversation so its reports show as links: the nodes to look at, the proposal to review.
import type { Session } from "../data/session.ts";
import type { AssistantAction, AssistantTarget, Because, ConversationEntry, Ended, TurnReply } from "../data/assistant.ts";

/**
 * Capabilities gating (ARCHITECTURE, Terms: Capabilities document): the panel is offered only
 * when the host's capabilities say it has an assistant and the host can reach one. The
 * in-browser host has neither, so it never shows the panel (I5: fully usable without it).
 */
export function assistantOffered(session: Pick<Session, "capabilities" | "host">): boolean {
  return session.capabilities.assistant && session.host.assistant !== undefined;
}

/** A turn sent from this tab: when its answer was kept, and what it answered. */
export interface KeptTurn {
  at: string;
  actions: AssistantAction[];
  ended: Ended;
}

/** One line of the conversation as the panel shows it. */
export type Entry =
  | { entry: "said"; at: string; text: string }
  | { entry: "reply"; at: string; text: string }
  | { entry: "report"; at: string; text: string }
  | { entry: "action"; at: string; action: AssistantAction }
  | { entry: "ended"; at: string; ended: Ended };

/** How many Cairn messages a turn keeps: one report per write, and a note when it ended without a reply. */
function reportCount(turn: KeptTurn): number {
  return turn.actions.length + (turn.ended.status === "replied" ? 0 : 1);
}

/**
 * The conversation as the panel shows it: `messages` as kept, oldest first, where a run of
 * Cairn's reports kept at the time a turn of `kept` was answered, and as many as it answered,
 * shows as that turn's writes and ending rather than as text.
 */
export function transcript(messages: readonly ConversationEntry[], kept: readonly KeptTurn[]): Entry[] {
  const entries: Entry[] = [];
  let at = 0;
  while (at < messages.length) {
    const message = messages[at];
    if (message === undefined) {
      break;
    }
    if (message.author !== "cairn") {
      entries.push({ entry: message.author === "user" ? "said" : "reply", at: message.at, text: message.content });
      at += 1;
      continue;
    }
    let end = at;
    while (messages[end]?.author === "cairn" && messages[end]?.at === message.at) {
      end += 1;
    }
    const run = messages.slice(at, end);
    const turn = kept.find((each) => each.at === message.at && reportCount(each) === run.length);
    if (turn === undefined) {
      entries.push(...run.map((report): Entry => ({ entry: "report", at: report.at, text: report.content })));
    } else {
      entries.push(...turn.actions.map((action): Entry => ({ entry: "action", at: message.at, action })));
      if (turn.ended.status !== "replied") {
        entries.push({ entry: "ended", at: message.at, ended: turn.ended });
      }
    }
    at = end;
  }
  return entries;
}

/** A turn the host answered but did not keep, shown after the first `after` kept messages. */
export interface Unsaved {
  after: number;
  entries: Entry[];
}

/**
 * The conversation as the panel shows it, with the turns the host answered but did not keep
 * each in its place: after the messages kept when it was answered.
 */
export function shown(messages: readonly ConversationEntry[], kept: readonly KeptTurn[], unsaved: readonly Unsaved[]): Entry[] {
  const entries: Entry[] = [];
  let from = 0;
  for (const turn of [...unsaved].sort((one, other) => one.after - other.after)) {
    const after = Math.min(Math.max(turn.after, from), messages.length);
    entries.push(...transcript(messages.slice(from, after), kept), ...turn.entries);
    from = after;
  }
  entries.push(...transcript(messages.slice(from), kept));
  return entries;
}

/**
 * When the latest turn's answer was kept: the time of the newest message, when it is not the
 * user's. A turn keeps its reports and its reply at one time, after the user's words.
 */
export function latestAnswerAt(messages: readonly ConversationEntry[]): string | undefined {
  const last = messages.at(-1);
  return last === undefined || last.author === "user" ? undefined : last.at;
}

/**
 * Whether `messages` hold the turn that said `text` and was answered `reply`: its words are
 * the newest the user said, followed by its answer when it had one. The host may answer a
 * turn it could not keep (its save ran out of time, or failed after a write).
 */
export function holdsTurn(messages: readonly ConversationEntry[], text: string, reply: TurnReply): boolean {
  let said = messages.length - 1;
  while (said >= 0 && messages[said]?.author !== "user") {
    said -= 1;
  }
  if (said < 0 || messages[said]?.content !== text) {
    return false;
  }
  const answered = (reply.actions ?? []).length > 0 || reply.reply != null || reply.ended.status !== "replied";
  return !answered || said < messages.length - 1;
}

/** A turn as the panel shows it from its answer alone, when the host did not keep it. */
export function answeredEntries(text: string, reply: TurnReply, at: string): Entry[] {
  const entries: Entry[] = [{ entry: "said", at, text }];
  entries.push(...(reply.actions ?? []).map((action): Entry => ({ entry: "action", at, action })));
  if (reply.ended.status !== "replied") {
    entries.push({ entry: "ended", at, ended: reply.ended });
  }
  if (reply.reply != null) {
    entries.push({ entry: "reply", at, text: reply.reply });
  }
  return entries;
}

/** A turn answered from this tab, as the panel keeps it. */
export function keptTurn(at: string, reply: TurnReply): KeptTurn {
  return { at, actions: reply.actions ?? [], ended: reply.ended };
}

/** The turns the tab keeps per conversation: the newest, at most this many. */
export const KEPT_TURN_COUNT_MAX = 50;

/** `kept` with `turn` added, newest last, within the limit. */
export function keeping(kept: readonly KeptTurn[], turn: KeptTurn): KeptTurn[] {
  return [...kept.filter((each) => each.at !== turn.at), turn].slice(-KEPT_TURN_COUNT_MAX);
}

/** What the panel says the conversation is about. */
export function aboutWords(target: AssistantTarget): string {
  return "journey" in target ? "this journey" : "this route's draft";
}

/** I5: why a write became a proposal, in a few words. */
export function becauseWords(because: Because): string {
  switch (because.reason) {
    case "structural":
      return "it changes structure, which goes through review";
    case "too_many_nodes":
      return because.count == null ? "it may touch every node" : `it touches ${String(because.count)} nodes, more than one change may`;
    case "too_many_nodes_this_turn":
      return `with this turn's other changes it would touch ${String(because.count)} nodes`;
    case "asked":
      return "the assistant drafted it for review";
  }
}

/** How a turn ended without a reply, in a few words. */
export function endedWords(ended: Ended): string {
  switch (ended.status) {
    case "replied":
      return "The assistant answered.";
    case "provider_timed_out":
      return "The model did not answer in time. Anything listed above was done; try again.";
    case "provider_failed":
      return `The model failed (${ended.message}). Anything listed above was done.`;
    case "iteration_limit":
      return "The assistant reached its limit of tool calls for one turn. Anything listed above was done.";
    case "turn_timed_out":
      return "The turn ran past its time limit. Anything listed above was done; a change still being written then may land after it, so check the journey.";
  }
}

/** A tool's name as words: `transition_node` reads "transition node". */
export function toolWords(tool: string): string {
  return tool.replaceAll("_", " ");
}
