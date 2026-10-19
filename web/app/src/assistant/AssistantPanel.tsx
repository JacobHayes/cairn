// The assistant panel (I5, I7; brief 5.8): the caller's conversation with the in-app
// assistant about a journey or a route's draft, in the inspector column's ASSISTANT tab. A
// message is one turn on the host; the reply shows with each write the assistant made: a
// direct change with the nodes it wrote and what it newly caused (D7), linked to those nodes,
// and a structural change, or one touching more than ten nodes, as a proposal to review in
// proposal review (C14), where applying it is the user's click. Offered only when the host's
// capabilities say there is an assistant (the in-browser host never has one). The panel opens
// from a toggle in the screen's header or the tab, and stays open across screens in this tab; the message
// being typed is a draft that survives a reload (ARCHITECTURE, Web UI).
import { useSelector } from "@xstate/react";
import { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { Link } from "react-router";

import { proposalAddress } from "../address.ts";
import { targetKey, type AssistantAction, type AssistantTarget } from "../data/assistant.ts";
import { useDraft } from "../data/drafts.ts";
import { linesOf, mayNotApply } from "../data/activity.ts";
import { useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { AssistantSlot, useAssistantDock } from "../shell/frame.tsx";
import { journeyPath } from "../journeys/address.ts";
import { Markdown } from "../ui/markdown.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { aboutWords, assistantOffered, becauseWords, endedWords, shown, toolWords, type Entry } from "./model.ts";
import { conversationView, sendTurn, type ConversationRef } from "./conversation.ts";
import "./assistant.css";
import { NODE_GONE } from "../ui/gone.ts";
import { ReadFailed } from "../ui/ReadFailed.tsx";

/** A node's title, when the screen knows it. */
export type TitleOf = (key: string) => string | undefined;

/** A journey's node titles, as its document holds them. */
export function journeyTitles(ready: Ready): TitleOf {
  const nodes = ready.journey.graph.nodes ?? [];
  return (key) => nodes.find((node) => node.key === key)?.title;
}

function NodeLink({ journey, node, titleOf }: { journey: string; node: string; titleOf: TitleOf | undefined }) {
  return (
    <Link to={journeyPath({ journey, node }, "graph")} data-testid="assistant-node" data-node={node}>
      {titleOf?.(node) ?? NODE_GONE}
    </Link>
  );
}

/** I5, D7: a direct change, with the nodes it wrote and what it newly caused. */
function Applied({ action, titleOf }: { action: Extract<AssistantAction, { outcome: "applied" }>; titleOf: TitleOf | undefined }) {
  const domain = action.receipt.domain;
  const journey = typeof domain === "object" && "journey" in domain ? domain.journey : undefined;
  const nodes = action.nodes ?? [];
  const lines = linesOf(action.consequences ?? {});
  return (
    <div className="assistant-write" data-testid="assistant-applied" data-tool={action.tool}>
      <span className="row">
        <Badge tone="good">Changed</Badge>
        <span>
          {toolWords(action.tool)}
        </span>
      </span>
      {journey === undefined || nodes.length === 0 ? null : (
        <span className="assistant-nodes">
          Wrote{" "}
          {nodes.map((node, index) => (
            <span key={node}>
              {index === 0 ? null : ", "}
              <NodeLink journey={journey} node={node} titleOf={titleOf} />
            </span>
          ))}
        </span>
      )}
      {lines.length === 0 ? (
        <span className="muted small" data-testid="assistant-caused" data-count="0">
          Nothing became stale, short, overdue, or stalled.
        </span>
      ) : (
        lines.map((line) => (
          <span key={`${line.journey}:${line.kind}`} className="muted small" data-testid="assistant-caused" data-kind={line.kind}>
            {line.kind === "stalled" ? (
              `The journey is now stalled`
            ) : (
              <>
                {line.kind === "undecided" ? mayNotApply(line.unanswered ?? [], (key) => titleOf?.(key) ?? NODE_GONE) : `Newly ${line.kind}`}:{" "}
                {line.nodes.map((node, index) => (
                  <span key={node}>
                    {index === 0 ? null : ", "}
                    <NodeLink journey={line.journey} node={node} titleOf={titleOf} />
                  </span>
                ))}
              </>
            )}
          </span>
        ))
      )}
    </div>
  );
}

/** I5, C14: a write drafted as a proposal, to review where the user applies it. */
function Proposed({ action }: { action: Extract<AssistantAction, { outcome: "proposed" }> }) {
  return (
    <div className="assistant-write" data-testid="assistant-proposed" data-proposal={action.proposal} data-because={action.because.reason}>
      <span className="row">
        <Badge tone="warn">Proposal</Badge>
        <span>
          {toolWords(action.tool)}, drafted for review: {becauseWords(action.because)}.
        </span>
      </span>
      <Link className="button primary" to={proposalAddress(action.destination, action.proposal)} data-testid="review-proposal">
        Review proposal
      </Link>
    </div>
  );
}

function Written({ action, titleOf }: { action: AssistantAction; titleOf: TitleOf | undefined }) {
  switch (action.outcome) {
    case "applied":
      return <Applied action={action} titleOf={titleOf} />;
    case "proposed":
      return <Proposed action={action} />;
    case "discarded":
      return (
        <div className="assistant-write muted small" data-testid="assistant-discarded" data-proposal={action.proposal}>
          Discarded proposal {action.proposal}.
        </div>
      );
  }
}

function EntryView({ entry, titleOf }: { entry: Entry; titleOf: TitleOf | undefined }) {
  switch (entry.entry) {
    case "said":
      return (
        <div className="assistant-said" data-testid="assistant-said">
          {entry.text}
        </div>
      );
    case "reply":
      return (
        <div className="assistant-reply" data-testid="assistant-reply">
          <Markdown text={entry.text} />
        </div>
      );
    case "report":
      return (
        <div className="assistant-report muted small" data-testid="assistant-report">
          <Markdown text={entry.text} />
        </div>
      );
    case "action":
      return <Written action={entry.action} titleOf={titleOf} />;
    case "ended":
      return (
        <div className="callout" data-testid="assistant-ended" data-status={entry.ended.status}>
          {endedWords(entry.ended)}
        </div>
      );
  }
}

/** The conversation's entries, oldest first. */
export function Transcript({ entries, titleOf }: { entries: readonly Entry[]; titleOf: TitleOf | undefined }) {
  return (
    <ol className="assistant-transcript" data-testid="assistant-transcript">
      {entries.map((entry, index) => (
        <li key={`${entry.at}:${String(index)}`}>
          <EntryView entry={entry} titleOf={titleOf} />
        </li>
      ))}
    </ol>
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

/** What the panel says before the first message: how a change is made (I5, I7); the composer's label says what to ask. */
function Empty() {
  return (
    <p className="muted" data-testid="assistant-empty">
      Small changes are made directly and reported here. Changes to structure, or to more than ten nodes, come back as a proposal for you to review.
    </p>
  );
}

/** The panel itself: the conversation, and the message being typed. */
function AssistantPanel({ target, titleOf }: { target: AssistantTarget; titleOf: TitleOf | undefined }) {
  const conversation = useConversation(target);
  return conversation === undefined ? null : <Conversing conversation={conversation} target={target} titleOf={titleOf} />;
}

function Conversing({ conversation, target, titleOf }: { conversation: ConversationRef; target: AssistantTarget; titleOf: TitleOf | undefined }) {
  const key = targetKey(target);
  const snapshot = useSelector(conversation, (each) => each);
  const { read, kept, unsaved, sending, failure } = useMemo(() => conversationView(snapshot), [snapshot]);
  const entries = useMemo(() => shown(read.status === "ready" ? read.conversation.messages : [], kept, unsaved), [read, kept, unsaved]);
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
        {read.status === "failed" ? <p className="callout callout-bad"><ReadFailed what="The conversation" error={read.message} /></p> : null}
        {read.status === "ready" && entries.length === 0 && sending === undefined ? <Empty /> : null}
        <Transcript entries={entries} titleOf={titleOf} />
        {sending === undefined ? null : (
          <>
            <div className="assistant-said" data-testid="assistant-said">
              {sending}
            </div>
            <p className="muted small" data-testid="assistant-working">
              The assistant is working on it...
            </p>
          </>
        )}
      </div>
      {failure === undefined ? null : (
        <p className="callout callout-bad" role="alert" data-testid="assistant-failed" data-status={failure.status}>
          {failure.status === 503 ? "The assistant is busy; try again in a moment." : `The message was not answered: ${failure.message}.`}
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
export function AssistantDock({ target, titleOf }: { target: AssistantTarget; titleOf?: TitleOf }) {
  const session = useSession();
  const [open, setOpen] = useDraft<boolean>("assistant-open");
  const assistant = session.host.assistant;
  const offered = assistantOffered(session) && assistant !== undefined;
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
      <AssistantPanel key={targetKey(target)} target={target} titleOf={titleOf} />
    </AssistantSlot>
  ) : null;
}
