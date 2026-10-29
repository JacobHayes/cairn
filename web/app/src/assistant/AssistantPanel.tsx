// The assistant panel (I5, I7): the caller's conversation with the in-app assistant about a
// journey or a route's draft, in the inspector column's ASSISTANT tab. A message is one turn on
// the host; the conversation shows as the host keeps it: the user's words, Cairn's receipt for
// each write, and the replies. A proposal the last turn drafted links to proposal review (C14),
// where applying it is the user's click. Offered only when the host's capabilities say there is
// an assistant (cairn demo never has one). The panel opens from a toggle in the screen's header
// or the tab, and stays open across screens in this tab; the message being typed is a draft that
// survives a reload (ARCHITECTURE, Web UI).
import { useSelector } from "@xstate/react";
import { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { Link } from "react-router";

import { proposalAddress } from "../address.ts";
import { targetKey, type AssistantTarget, type ConversationEntry, type Proposed } from "../data/assistant.ts";
import { valueOf } from "../data/keyed-read.ts";
import { useDraft } from "../data/drafts.ts";
import { useLive, useSession } from "../data/react.ts";
import { AssistantSlot, useAssistantDock } from "../shell/frame.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { Button } from "../ui/kit.tsx";
import { listKey, proposalsUnder } from "../proposals/read.ts";
import { aboutWords } from "./model.ts";
import { conversationView, sendTurn, type ConversationRef } from "./conversation.ts";
import "./assistant.css";
import { Failed, kindOf } from "../ui/Failed.tsx";

function EntryView({ entry }: { entry: ConversationEntry }) {
  switch (entry.author) {
    case "user":
      return (
        <div className="assistant-said" data-testid="assistant-said">
          {entry.content}
        </div>
      );
    case "assistant":
      return (
        <div className="assistant-reply" data-testid="assistant-reply">
          <Markdown text={entry.content} />
        </div>
      );
    case "cairn":
      return (
        <p className="assistant-receipt muted small" data-testid="assistant-report">
          {entry.content}
        </p>
      );
  }
}

/** The conversation's entries, oldest first. */
export function Transcript({ entries }: { entries: readonly ConversationEntry[] }) {
  return (
    <ol className="assistant-transcript" data-testid="assistant-transcript" aria-live="polite">
      {entries.map((entry, index) => (
        <li key={`${entry.at}:${String(index)}`}>
          <EntryView entry={entry} />
        </li>
      ))}
    </ol>
  );
}

/** I5, C14: the open proposals under the conversation's destination, by name, each linking to its review where applying it is the user's click. Read from the destination's list, so they are there after a reload and drop out once applied or discarded. */
function OpenProposals({ target }: { target: AssistantTarget }) {
  const destination = target;
  const { view } = useLive(listKey(destination), proposalsUnder(destination));
  const items = valueOf(view)?.items ?? [];
  return items.map((proposal) => (
    <p key={proposal.id} data-testid="assistant-proposed" data-proposal={proposal.id}>
      <Link className="button" to={proposalAddress(proposal.destination, proposal.id)} data-testid="review-proposal">
        Review: {proposal.title}
      </Link>
    </p>
  ));
}

/** I5, C14: a proposal the last turn drafted under another destination (a journey saved as a route), to review where the user applies it. */
function ElsewhereLink({ proposed }: { proposed: Proposed }) {
  return (
    <p data-testid="assistant-proposed" data-proposal={proposed.proposal}>
      <Link className="button" to={proposalAddress(proposed.destination, proposed.proposal)} data-testid="review-proposal">
        Review proposal
      </Link>
    </p>
  );
}

/** I5: `target`'s conversation as the tab holds it (the session's conversation manager), read again as the panel opens; none until the manager has started it. */
function useConversation(target: AssistantTarget): ConversationRef | undefined {
  const { conversations } = useSession();
  const key = targetKey(target);
  // Before paint, so the panel never shows a frame without its conversation.
  useLayoutEffect(() => {
    conversations.send({ type: "open", target });
    // `key` names the target.
  }, [conversations, key]);
  return useSelector(conversations, (snapshot) => snapshot.context.conversations[key]);
}

/** The message being typed (a draft kept across reloads) under its label, which says what to ask, and sending it. */
function Composer({ target, ready, sending, send }: { target: AssistantTarget; ready: boolean; sending: boolean; send: (text: string) => Promise<boolean> }) {
  const draftKey = `assistant:${targetKey(target)}`;
  const [draft, setDraft] = useDraft<string>(draftKey);
  const text = (draft ?? "").trim();
  const submit = async () => {
    if (text === "" || sending || !ready) {
      return;
    }
    setDraft(undefined);
    if (!(await send(text))) {
      setDraft(text);
    }
  };
  return (
    <form
      className="stack"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <label className="stack assistant-ask">
        <span>Ask about {aboutWords(target)}, or ask for a change</span>
        <textarea
          data-testid="assistant-message"
          value={draft ?? ""}
          disabled={sending}
          onChange={(event) => {
            setDraft(event.target.value === "" ? undefined : event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              void submit();
            }
          }}
        />
      </label>
      <span className="row">
        <Button primary type="submit" data-testid="assistant-send" disabled={sending || text === "" || !ready}>
          Send
        </Button>
        <span className="muted small assistant-keys">
          <kbd>Ctrl+Enter</kbd> sends.
        </span>
      </span>
    </form>
  );
}

/** What the panel offers before the first message: one request to start from. */
function Empty({ target }: { target: AssistantTarget }) {
  return (
    <p className="muted" data-testid="assistant-empty">
      {"journey" in target ? "Try: \u201CWhat should I do next?\u201D" : "Try: \u201CWhat does this draft still need?\u201D"}
    </p>
  );
}

/** The panel itself: the conversation, and the message being typed. */
function AssistantPanel({ target }: { target: AssistantTarget }) {
  const conversation = useConversation(target);
  return conversation === undefined ? null : <Conversing conversation={conversation} target={target} />;
}

function Conversing({ conversation, target }: { conversation: ConversationRef; target: AssistantTarget }) {
  const key = targetKey(target);
  const snapshot = useSelector(conversation, (each) => each);
  const { read, elsewhere, sending, failure } = useMemo(() => conversationView(snapshot), [snapshot]);
  const entries = useMemo(() => (read.status === "ready" ? read.conversation.messages : []), [read]);
  const send = (text: string) => sendTurn(conversation, text);
  const scroller = useRef<HTMLDivElement>(null);
  // The newest turn in view: the transcript scrolls itself, never the page around it.
  useEffect(() => {
    scroller.current?.scrollTo({ top: scroller.current.scrollHeight });
  }, [entries.length, sending]);
  return (
    <section className="panel assistant-panel" aria-label="Assistant" data-testid="assistant-panel" data-target={key} data-status={sending === undefined ? "idle" : "working"}>
      <div ref={scroller} className="assistant-scroll inspector-body">
        {read.status === "loading" ? <p className="muted small">Reading the conversation...</p> : null}
        {read.status === "failed" ? (
          <p className="callout callout-bad">
            <Failed what="The conversation" error={read.message} />{" "}
            <Button type="button" data-testid="assistant-retry" onClick={() => { conversation.send({ type: "refresh" }); }}>
              Try again
            </Button>
          </p>
        ) : null}
        {read.status === "ready" && entries.length === 0 && sending === undefined ? <Empty target={target} /> : null}
        <Transcript entries={entries} />
        <OpenProposals target={target} />
        {elsewhere.map((proposed) => (
          <ElsewhereLink key={proposed.proposal} proposed={proposed} />
        ))}
        {sending === undefined ? null : (
          <>
            <div className="assistant-said" data-testid="assistant-said">
              {sending}
            </div>
            <p className="muted small" role="status" data-testid="assistant-working">
              The assistant is working on it...
            </p>
          </>
        )}
      </div>
      {failure === undefined ? null : (
        <p className="callout callout-bad" role="alert" data-testid="assistant-failed" data-status={failure.status}>
          {failure.status === 0 ? (
            failure.message
          ) : failure.status === 503 ? (
            "The assistant is busy; try again in a moment."
          ) : failure.status === 409 ? (
            "A reply is still in progress in this conversation; wait for it before sending another message."
          ) : (
            <Failed what="The message" verb="answered" error={kindOf(failure)} />
          )}
        </p>
      )}
      <Composer target={target} ready={read.status === "ready"} sending={sending !== undefined} send={send} />
    </section>
  );
}

/**
 * The panel when open, and its opener for the inspector column's tabs: nothing at all when the host
 * offers no assistant (capabilities gating).
 */
export function AssistantDock({ target }: { target: AssistantTarget }) {
  const session = useSession();
  const [open, setOpen] = useDraft<boolean>("assistant-open");
  const offered = session.capabilities.assistant;
  const dock = useMemo(
    () => ({
      open: () => {
        setOpen(true);
      },
      close: () => {
        setOpen(undefined);
      },
    }),
    [setOpen],
  );
  useAssistantDock(offered ? dock : undefined);
  if (!offered) {
    return null;
  }
  return open === true ? (
    <AssistantSlot>
      <AssistantPanel key={targetKey(target)} target={target} />
    </AssistantSlot>
  ) : null;
}
