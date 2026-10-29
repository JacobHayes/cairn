// The sync dot at the right edge of the brand row at the top of the rail (design 9): one button that is
// only a mark (its colour and shape and its accessible name carry the words) that says whether what you see is what exists and whether your edits
// are landing. Hovering or focusing it peeks the popover; clicking pins it open. It holds the revision,
// engine and today, the release the server runs, what needs you, and this tab's recent saves. Nothing on it closes itself
// or needs closing but the popover, which Esc dismisses. A polite and an assertive live
// region announce the changes that matter.
import { WriteProblem } from "../ui/Failed.tsx";
import { useActorRef, useSelector } from "@xstate/react";
import { useEffect, useId, useLayoutEffect, useRef } from "react";
import { Link, useNavigate } from "react-router";

import { pathTo } from "../address.ts";
import type { SaveEvent } from "../data/activity.ts";
import { useCapabilities, useDerivation, useProblems, useSaves, useSavedAnnouncement, useSession, useSettled, useSkew, useStreamStatus, useSync } from "../data/react.ts";
import { dateWords } from "../status/when.ts";
import type { Problem, SyncSummary } from "../data/sync.ts";
import { Button } from "../ui/kit.tsx";
import { place, windowBounds } from "../ui/place.ts";
import { popoverAnchor, popoverPinned, popoverShown, syncPopover, type Anchor } from "./sync-popover.ts";

function NeedsYou({ problems, onGo }: { problems: readonly Problem[]; onGo: () => void }) {
  const navigate = useNavigate();
  const { edits } = useSession();
  const skew = useSkew();
  return (
    <section className="sync-section" aria-label="Needs you">
      <h3>Needs you</h3>
      <ul>
        {problems.map((problem) => (
          <li key={problem.id} className="stack" data-testid="sync-problem" data-kind={problem.kind}>
            <span>
              {problem.label === undefined ? null : <strong>{problem.label}: </strong>}
              <WriteProblem message={problem.message} detail={problem.detail} />
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
              {problem.retry && !skew ? (
                <Button
                  onClick={() => {
                    edits.send({ type: "retry", id: problem.id });
                  }}
                >
                  Retry
                </Button>
              ) : null}
              <Button
                onClick={() => {
                  edits.send({ type: "discard", id: problem.id });
                }}
              >
                Discard
              </Button>
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Recent({ saves, onGo }: { saves: readonly SaveEvent[]; onGo: () => void }) {
  const navigate = useNavigate();
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
              {save.address === undefined ? null : (
                <>
                  {" "}
                  <Button
                    onClick={() => {
                      void navigate(save.address ?? "/");
                      onGo();
                    }}
                  >
                    Review it
                  </Button>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** The release the server runs, as the head shows it: `v1.2.3`, or `dev build` for a build without one. */
function versionWords(version: string): string {
  return version === "dev" ? "dev build" : `v${version}`;
}

/** The head: the popover's name, which labels it, and at the right the release the server runs. */
function Head({ heading, version }: { heading: string; version: string }) {
  return (
    <div className="popover-head sync-head">
      <span id={heading}>Sync</span>
      <span className="sync-version" data-testid="sync-version">
        {version}
      </span>
    </div>
  );
}

function Popover({ summary, anchor, onClose }: { summary: SyncSummary; anchor: Anchor; onClose: () => void }) {
  const stream = useStreamStatus();
  const problems = useProblems();
  const saves = useSaves();
  const derivation = useDerivation();
  const skew = useSkew();
  const version = versionWords(useCapabilities().version);
  const heading = useId();
  // Just below the dot, kept inside the window (it is fixed, so the rail's scroller does not clip it).
  const popover = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const element = popover.current;
    if (element === null) {
      return;
    }
    element.style.setProperty("left", "0px");
    element.style.setProperty("top", "0px");
    const box = element.getBoundingClientRect();
    const at = place(anchor, box, windowBounds(), { side: "bottom", align: "start", gap: 8, margin: 12 });
    element.style.setProperty("left", `${String(at.left)}px`);
    element.style.setProperty("top", `${String(at.top)}px`);
  }, [anchor]);
  return (
    <div ref={popover} className="popover sync-popover" role="dialog" aria-labelledby={heading} tabIndex={-1} data-popover="" data-overhang="" data-testid="sync-popover">
      <Head heading={heading} version={version} />
      <div className="popover-body stack">
        <p>
          <strong>{summary.label}</strong>
          <span className="muted"> · live updates {stream === "live" ? "on" : stream === "reconnecting" ? "paused" : "off"}</span>
        </p>
        {derivation === undefined ? null : (
          <p className="mono small muted">
            Today {dateWords(derivation.today, derivation.today)}
          </p>
        )}
        {/* On a phone the top bar has no You tab: who you are is one tap from here. */}
        <Link to={pathTo("me")} className="sync-you" onClick={onClose}>
          You: who you are here &rsaquo;
        </Link>
        <Link to={pathTo("agents")} className="sync-you" onClick={onClose}>
          Connect an agent &rsaquo;
        </Link>
        {skew && (
          <p className="stack" data-testid="sync-skew">
            <span>Cairn was updated. Reload to keep saving.</span>
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
        <Recent saves={saves} onGo={onClose} />
      </div>
    </div>
  );
}

/**
 * The popover's state, the machine's (sync-popover.ts): pinned by a click or peeked by hover or
 * focus, visible in either. The document listeners stay here: Esc, a press outside and a resize
 * close it, and a pinned one takes focus and gives it back to the chip.
 */
function usePopover() {
  const ref = useActorRef(syncPopover);
  const shown = useSelector(ref, popoverShown);
  const pinned = useSelector(ref, popoverPinned);
  const anchor = useSelector(ref, popoverAnchor);
  const wrap = useRef<HTMLSpanElement>(null);
  const chip = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!shown) {
      return undefined;
    }
    if (pinned) {
      wrap.current?.querySelector<HTMLElement>(".sync-popover")?.focus();
    }
    const close = () => {
      ref.send({ type: "close" });
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        const wasPinned = popoverPinned(ref.getSnapshot());
        close();
        if (wasPinned) {
          chip.current?.focus();
        }
      }
    };
    const onPointer = (event: PointerEvent) => {
      if (event.target instanceof Node && wrap.current?.contains(event.target) !== true) {
        close();
      }
    };
    // It is placed from the dot's box when it opens, so a resize closes it rather than leaving it adrift.
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    globalThis.addEventListener("resize", close);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
      globalThis.removeEventListener("resize", close);
    };
  }, [ref, shown, pinned]);
  return { ref, shown, pinned, anchor, wrap, chip };
}

export function SyncChip() {
  const session = useSession();
  const summary = useSync();
  const quiet = useSettled();
  const derivation = useDerivation();
  const { ref, shown, pinned, anchor, wrap, chip } = usePopover();
  const saved = useSavedAnnouncement();
  // Hover and keyboard focus show the same popover without pinning it: a click pins it, and
  // leaving or blurring lets a peeked one go, after a beat so the pointer can cross into it.
  const show = () => {
    if (chip.current !== null) {
      ref.send({ type: "peek", anchor: chip.current.getBoundingClientRect() });
    }
  };
  const hide = () => {
    ref.send({ type: "unpeek" });
  };
  return (
    <span className="sync-wrap" ref={wrap} onPointerEnter={show} onPointerLeave={hide} onFocus={(event) => { if (!pinned && event.target === chip.current && chip.current.matches(":focus-visible")) { show(); } }} onBlur={(event) => { if (!(event.relatedTarget instanceof Node && wrap.current?.contains(event.relatedTarget) === true)) { hide(); } }}>
      <button
        ref={chip}
        type="button"
        className="sync"
        data-testid="sync"
        data-state={summary.state}
        data-tone={summary.tone}
        data-revision={derivation?.revision}
        data-deployment={derivation?.deployment}
        data-today={derivation?.today}
        data-settled={quiet}
        aria-label={`${summary.label}. ${summary.sentence}`}
        aria-haspopup="dialog"
        aria-expanded={shown}
        onClick={(event) => {
          if (summary.state === "new-version") {
            location.reload();
          } else if (summary.state === "behind") {
            // A refetch failed or is slow: the stream's current revisions ask every view again.
            session.root.send({ type: "reopen" });
          } else {
            ref.send({ type: "pin", anchor: event.currentTarget.getBoundingClientRect() });
          }
        }}
      />
      {shown ? (
        <Popover
          summary={summary}
          anchor={anchor}
          onClose={() => {
            ref.send({ type: "close" });
          }}
        />
      ) : null}
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
