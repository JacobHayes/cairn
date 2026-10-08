// The assistant panel (I5, I7; brief 5.8): the caller's conversation with the in-app
// assistant about a journey or a route's draft, beside the canvas, overview, or draft. A
// message is one turn on the host; the reply shows with each write the assistant made: a
// direct change with the nodes it wrote and what it newly caused (D7), linked to those nodes,
// and a structural change, or one touching more than ten nodes, as a proposal to review in
// proposal review (C14), where applying it is the user's click. Offered only when the host's
// capabilities say there is an assistant (the in-browser host never has one). The panel opens
// from a toggle in the screen's header and stays open across screens in this tab; the message
// being typed is a draft that survives a reload (ARCHITECTURE, Web UI).
import { useEffect, useRef, useSyncExternalStore } from "react";
import { Link } from "react-router";

import { targetKey, type AssistantAction, type AssistantHost, type AssistantTarget } from "../data/assistant.ts";
import { useDraft } from "../data/drafts.ts";
import { linesOf } from "../data/notices.ts";
import { useSession } from "../data/react.ts";
import type { Ready } from "../detail/model.ts";
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
        <span className="muted" data-testid="assistant-caused" data-count="0">
          Nothing became stale, short, overdue, or stalled.
        </span>
      ) : (
        lines.map((line) => (
          <span key={`${line.journey}:${line.kind}`} className="muted" data-testid="assistant-caused" data-kind={line.kind}>
            {line.kind === "stalled" ? (
              `The journey is now stalled`
            ) : (
              <>
                Newly {line.kind}:{" "}
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
      <Link className="button button-primary" to={`/proposals/${action.proposal}`} data-testid="review-proposal">
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
        <div className="assistant-write muted" data-testid="assistant-discarded" data-proposal={action.proposal}>
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
        <div className="assistant-report muted" data-testid="assistant-report">
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
        className="textarea"
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
        <span className="muted">Ctrl+Enter sends.</span>
      </span>
    </form>
  );
}

/** What the panel says before the first message. */
function Empty({ target }: { target: AssistantTarget }) {
  return (
    <p className="muted" data-testid="assistant-empty">
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
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [entries.length, sending]);
  return (
    <aside className="assistant-panel panel" aria-label="Assistant" data-testid="assistant-panel" data-target={key} data-status={sending === undefined ? "idle" : "working"}>
      <div className="row">
        <strong>Assistant</strong>
        <span className="muted">about {aboutWords(target)}</span>
        <span className="shell-spacer" />
        <Button aria-label="Close the assistant" onClick={onClose}>
          ×
        </Button>
      </div>
      <div className="assistant-scroll">
        {read.status === "loading" ? <p className="muted">Reading the conversation...</p> : null}
        {read.status === "failed" ? <p className="callout callout-bad">The conversation could not be read: {read.message}</p> : null}
        {read.status === "ready" && entries.length === 0 && sending === undefined ? <Empty target={target} /> : null}
        <Transcript entries={entries} titleOf={titleOf} />
        {sending === undefined ? null : (
          <>
            <div className="assistant-said" data-testid="assistant-said">
              {sending}
            </div>
            <p className="muted" data-testid="assistant-working">
              The assistant is working on it...
            </p>
          </>
        )}
        <div ref={end} />
      </div>
      {failure === undefined ? null : (
        <p className="callout callout-bad" role="alert" data-testid="assistant-failed" data-status={failure.status}>
          {failure.status === 503 ? "The assistant is busy; try again in a moment." : `The message was not answered: ${failure.message}.`}
        </p>
      )}
      <Composer draftKey={`assistant:${key}`} ready={read.status === "ready"} sending={sending !== undefined} send={send} />
    </aside>
  );
}

/**
 * The panel's toggle, for a screen's header, and the panel when open: nothing at all when
 * the host offers no assistant (capabilities gating).
 */
export function AssistantDock({ target, titleOf }: { target: AssistantTarget; titleOf?: TitleOf }) {
  const session = useSession();
  const [open, setOpen] = useDraft<boolean>("assistant-open");
  const assistant = session.host.assistant;
  if (!assistantOffered(session) || assistant === undefined) {
    return null;
  }
  return (
    <>
      <Button
        className="assistant-toggle"
        data-testid="assistant-toggle"
        aria-pressed={open === true}
        onClick={() => {
          setOpen(open === true ? undefined : true);
        }}
      >
        Assistant
      </Button>
      {open === true ? (
        <AssistantPanel
          key={targetKey(target)}
          assistant={assistant}
          target={target}
          titleOf={titleOf}
          onClose={() => {
            setOpen(undefined);
          }}
        />
      ) : null}
    </>
  );
}
