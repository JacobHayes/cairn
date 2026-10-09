// The sync chip at the strip's right end (design 9): one button with a mark and an
// always-visible label that says whether what you see is what exists and whether your edits
// are landing. Hovering shows one sentence; clicking opens the popover with the revision,
// engine and today, what needs you, and this tab's recent saves. Nothing on it closes itself
// or needs closing but the popover, which Esc dismisses. A polite and an assertive live
// region announce the changes that matter.
import { useEffect, useId, useRef, useState } from "react";
import { useNavigate } from "react-router";

import type { SaveEvent } from "../data/activity.ts";
import { useDerivation, useSaves, useSession, useSkew, useStreamStatus, useSync } from "../data/react.ts";
import { dateWords } from "../timeline/model.ts";
import { SYNC_SAVED_MS, type Problem, type SyncSummary } from "../data/sync.ts";
import { Button } from "../ui/kit.tsx";

/** A save is announced as it lands, whatever the chip says at the moment: "Saved", and the receipt's warning. */
function useSavedAnnouncement(): { id: number; text: string } | undefined {
  const latest = useSaves()[0];
  const id = latest?.id;
  const [spoken, setSpoken] = useState<{ id: number; text: string } | undefined>();
  useEffect(() => {
    if (id === undefined) {
      return undefined;
    }
    setSpoken({ id, text: ["Saved", latest?.warning].filter(Boolean).join(". ") });
    const timer = setTimeout(() => {
      setSpoken(undefined);
    }, SYNC_SAVED_MS);
    return () => {
      clearTimeout(timer);
    };
    // A new save is a new id; its warning comes with it.
  }, [id]);
  return spoken;
}

function NeedsYou({ problems, onGo }: { problems: readonly Problem[]; onGo: () => void }) {
  const navigate = useNavigate();
  return (
    <section className="sync-section" aria-label="Needs you">
      <h3>Needs you</h3>
      <ul>
        {problems.map((problem, at) => (
          <li key={at} className="stack" data-testid="sync-problem" data-kind={problem.kind}>
            <span>
              {problem.label === undefined ? null : <strong>{problem.label}: </strong>}
              {problem.message}
            </span>
            <span className="row">
              {problem.address === undefined ? null : (
                <Button
                  onClick={() => {
                    void navigate(problem.address ?? "/");
                    onGo();
                  }}
                >
                  Go to it
                </Button>
              )}
              <Button onClick={problem.discard}>Discard</Button>
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Recent({ saves }: { saves: readonly SaveEvent[] }) {
  return (
    <section className="sync-section" aria-label="Recent">
      <h3>Recent</h3>
      {saves.length === 0 ? (
        <p className="muted small">Nothing saved in this tab yet.</p>
      ) : (
        <ul>
          {saves.map((save) => (
            <li key={save.id} data-testid="sync-recent">
              <span className="mono small muted">{save.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })}</span> {save.text}
              {save.warning === undefined ? null : <span className="small muted sync-warning">{save.warning}</span>}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function Popover({ summary, problems, onClose }: { summary: SyncSummary; problems: readonly Problem[]; onClose: () => void }) {
  const session = useSession();
  const stream = useStreamStatus();
  const saves = useSaves();
  const derivation = useDerivation();
  const skew = useSkew();
  const heading = useId();
  return (
    <div className="popover sync-popover" role="dialog" aria-labelledby={heading} tabIndex={-1} data-popover="" data-testid="sync-popover">
      <div className="popover-head" id={heading}>
        Sync
      </div>
      <div className="popover-body stack">
        <p>
          <strong>{summary.label}</strong>
          <span className="muted"> · live updates {stream === "live" ? "on" : stream === "reconnecting" ? "paused" : "off"}</span>
        </p>
        {derivation === undefined ? null : (
          <p className="mono small muted" title={`Engine ${session.host.engineVersion}`}>
            Revision {derivation.revision} · Today {dateWords(derivation.today, derivation.today)}
          </p>
        )}
        {skew === undefined ? null : (
          <p className="stack" data-testid="sync-skew">
            <span>Cairn was updated. Reload to keep saving; your unsent edits are kept.</span>
            <span className="row">
              <Button
                primary
                onClick={() => {
                  location.reload();
                }}
              >
                Reload
              </Button>
            </span>
          </p>
        )}
        {problems.length === 0 ? null : <NeedsYou problems={problems} onGo={onClose} />}
        <Recent saves={saves} />
      </div>
    </div>
  );
}

/** The popover's open state: Esc and a press outside close it, and focus goes in and back to the chip. */
function usePopover() {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);
  const chip = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) {
      return undefined;
    }
    wrap.current?.querySelector<HTMLElement>(".sync-popover")?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        chip.current?.focus();
      }
    };
    const onPointer = (event: PointerEvent) => {
      if (event.target instanceof Node && wrap.current?.contains(event.target) !== true) {
        setOpen(false);
      }
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, [open]);
  return { open, setOpen, wrap, chip };
}

export function SyncChip() {
  const session = useSession();
  const { summary, problems } = useSync();
  const derivation = useDerivation();
  const { open, setOpen, wrap, chip } = usePopover();
  const saved = useSavedAnnouncement();
  return (
    <span className="sync-wrap" ref={wrap}>
      <button
        ref={chip}
        type="button"
        className="sync"
        data-testid="sync"
        data-host={session.host.kind}
        data-state={summary.state}
        data-tone={summary.tone}
        data-revision={derivation?.revision}
        data-deployment={derivation?.deployment}
        data-today={derivation?.today}
        title={summary.sentence}
        aria-label={`${summary.label}. ${summary.sentence}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => {
          if (summary.state === "new-version") {
            location.reload();
          } else if (summary.state === "behind") {
            // A refetch failed or is slow: the stream's current revisions ask every view again.
            session.subscription.reopen();
          } else {
            setOpen(!open);
          }
        }}
      >
        {summary.label}
      </button>
      {open ? <Popover summary={summary} problems={problems} onClose={() => { setOpen(false); }} /> : null}
      <span className="visually-hidden" role="status" aria-live="polite">
        {saved === undefined ? null : <span key={saved.id}>{saved.text}</span>}
        {summary.announce === "polite" ? summary.sentence : ""}
      </span>
      <span className="visually-hidden" role="alert" aria-live="assertive">
        {summary.announce === "assertive" ? summary.sentence : ""}
      </span>
    </span>
  );
}
