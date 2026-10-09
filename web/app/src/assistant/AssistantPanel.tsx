// The assistant panel (I5, I7; brief 5.8): the caller's conversation with the in-app
// assistant about a journey or a route's draft, in the inspector column's ASSISTANT tab. A
// message is one turn on the host; the reply shows with each write the assistant made: a
// direct change with the nodes it wrote and what it newly caused (D7), linked to those nodes,
// and a structural change, or one touching more than ten nodes, as a proposal to review in
// proposal review (C14), where applying it is the user's click. Offered only when the host's
// capabilities say there is an assistant (the in-browser host never has one). The panel opens
// from a toggle in the screen's header or the tab, and stays open across screens in this tab; the message
// being typed is a draft that survives a reload (ARCHITECTURE, Web UI).
import { useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { Link } from "react-router";

import { targetKey, type AssistantAction, type AssistantHost, type AssistantTarget } from "../data/assistant.ts";
import { useDraft } from "../data/drafts.ts";
import { linesOf, mayNotApply } from "../data/notices.ts";
import { useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
import { AssistantSlot, useAssistantDock, useFrameActions, useFrameState } from "../shell/frame.tsx";
import { Markdown } from "../ui/markdown.tsx";
import { Badge, Button } from "../ui/kit.tsx";
import { aboutWords, assistantOffered, becauseWords, endedWords, shown, toolWords, type Entry } from "./model.ts";
import { conversationOf } from "./conversation.ts";
import "./assistant.css";

/** A node's title, when the screen knows it. */
export type TitleOf = (key: string) => string | undefined;

/** A journey's node titles, as its document holds them. */
export function journeyTitles(ready: Ready): TitleOf {
  const nodes = ready.journey.graph.nodes ?? [];
  return (key) => nodes.find((node) => node.key === key)?.title;
}

function NodeLink({ journey, node, titleOf }: { journey: string; node: string; titleOf: TitleOf | undefined }) {
  return (
    <Link to={`/journeys/${journey}/nodes/${node}`} data-testid="assistant-node" data-node={node}>
      {titleOf?.(node) ?? node}
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
          {toolWords(action.tool)}, now at revision {action.receipt.revision}
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
                {line.kind === "undecided" ? mayNotApply(line.unanswered ?? [], (key) => titleOf?.(key) ?? key) : `Newly ${line.kind}`}:{" "}
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
      <Link className="button primary" to={`/proposals/${action.proposal}`} data-testid="review-proposal">
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

/** I5: `target`'s conversation as the tab holds it, read again as the panel opens. */
function useConversation(assistant: AssistantHost, target: AssistantTarget) {
  const conversation = conversationOf(assistant, target);
  useEffect(() => {
    conversation.refresh();
  }, [conversation]);
  const view = useSyncExternalStore(conversation.subscribe, () => conversation.view);
  const entries = view.read.status === "ready" ? shown(view.read.conversation.messages, view.kept, view.unsaved) : shown([], [], view.unsaved);
  return { ...view, entries, send: (text: string) => conversation.send(text) };
}

/** The message being typed (a draft kept across reloads), and sending it. */
function Composer({ draftKey, ready, sending, send }: { draftKey: string; ready: boolean; sending: boolean; send: (text: string) => Promise<boolean> }) {
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
      <textarea
        aria-label="Message to the assistant"
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
      <span className="row">
        <Button primary type="submit" data-testid="assistant-send" disabled={sending || text === "" || !ready}>
          Send
        </Button>
        <span className="muted small">Ctrl+Enter sends.</span>
      </span>
    </form>
  );
}

/** What the panel says before the first message. */
function Empty({ target }: { target: AssistantTarget }) {
  return (
    <p className="muted small" data-testid="assistant-empty">
      Ask about {aboutWords(target)}, or ask for a change. Changes to state (an answer, a transition, a note, an assignment, a pin,
      a snooze) are made directly and reported here; changes to structure, and any change touching more than ten nodes, come back
      as a proposal for you to review and apply. You can always ask for a proposal instead.
    </p>
  );
}

/** The panel itself: the conversation, and the message being typed. */
function AssistantPanel({ assistant, target, titleOf, onClose }: { assistant: AssistantHost; target: AssistantTarget; titleOf: TitleOf | undefined; onClose: () => void }) {
  const key = targetKey(target);
  const { read, entries, sending, failure, send } = useConversation(assistant, target);
  const scroller = useRef<HTMLDivElement>(null);
  // The newest turn in view: the transcript scrolls itself, never the page around it.
  useEffect(() => {
    scroller.current?.scrollTo({ top: scroller.current.scrollHeight });
  }, [entries.length, sending]);
  return (
    <section className="assistant-panel" aria-label="Assistant" data-testid="assistant-panel" data-target={key} data-status={sending === undefined ? "idle" : "working"}>
      <div className="row assistant-head">
        <span className="muted small">About {aboutWords(target)}</span>
        <span className="spacer" />
        <Button aria-label="Close the assistant" onClick={onClose}>
          ×
        </Button>
      </div>
      <div ref={scroller} className="assistant-scroll inspector-body">
        {read.status === "loading" ? <p className="muted small">Reading the conversation...</p> : null}
        {read.status === "failed" ? <p className="callout callout-bad">The conversation could not be read: {read.message}</p> : null}
        {read.status === "ready" && entries.length === 0 && sending === undefined ? <Empty target={target} /> : null}
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
      <Composer draftKey={`assistant:${key}`} ready={read.status === "ready"} sending={sending !== undefined} send={send} />
    </section>
  );
}

/**
 * The panel's toggle, for a screen's header, and the panel when open: nothing at all when
 * the host offers no assistant (capabilities gating).
 */
export function AssistantDock({ target, titleOf }: { target: AssistantTarget; titleOf?: TitleOf }) {
  const session = useSession();
  const [open, setOpen] = useDraft<boolean>("assistant-open");
  const frame = useFrameState();
  const actions = useFrameActions();
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
  // Open but behind the inspector tab: the toggle brings it forward, and only then closes it.
  const inView = open === true && (frame === undefined || frame.tab === "assistant");
  return (
    <>
      <Button
        className="assistant-toggle"
        data-testid="assistant-toggle"
        aria-pressed={inView}
        onClick={() => {
          if (open !== true) {
            setOpen(true);
          } else if (inView) {
            setOpen(undefined);
          } else {
            actions?.show("assistant");
          }
        }}
      >
        Assistant
      </Button>
      {open === true ? (
        <AssistantSlot>
          <AssistantPanel
            key={targetKey(target)}
            assistant={assistant}
            target={target}
            titleOf={titleOf}
            onClose={() => {
              setOpen(undefined);
            }}
          />
        </AssistantSlot>
      ) : null}
    </>
  );
}
