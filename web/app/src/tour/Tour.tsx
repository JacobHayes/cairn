// PRD: C20.
// The tour of Cairn on screen: the provider that owns the tour's machine and the cursor, and the
// welcome, continue, failure and end panels; a step is the spotlight overlay (Overlay.tsx). What is true
// (the record, the practice journey, which phase the run is in) is the machine's (machine.ts);
// what is shown for it is a pure choice (model.ts, `surfaceOf`); what a step says and when it is
// done are steps.ts. This file only draws the panels and moves the person between steps.
import { useActorRef, useSelector } from "@xstate/react";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import { assistantOffered } from "../assistant/model.ts";
import { useJourney, useRetryJourney, useSession } from "../data/react.ts";
import { useDraft } from "../data/drafts.ts";
import type { Tour as TourRecord } from "../data/host.ts";
import { pathTo } from "../address.ts";
import { HOME_PATH, journeyPath } from "../journeys/address.ts";
import { overlayOpen, typing } from "../ui/typing.ts";
import { Button } from "../ui/kit.tsx";
import { tourMachine, type TourEvent, type TourSnapshot } from "./machine.ts";
import { eligible, phaseOf, surfaceOf, type Phase, type Surface, type SurfaceInput } from "./model.ts";
import { tourPorts } from "./ports.ts";
import { Overlay } from "./Overlay.tsx";
import { FramingContext, type Framing } from "./presentation.ts";
import { clampCursor, stepsFor, type Step } from "./steps.ts";
import { locate, underJourney } from "./where.ts";
import "./tour.css";

/** What the rest of the app may ask of the tour. */
export interface TourApi {
  snapshot: TourSnapshot;
  send: (event: TourEvent) => void;
  surface: Surface;
  phase: Phase;
  /** The record as last read or written. */
  record: TourRecord | undefined;
  /** The practice journey of the run in hand. */
  run: string | undefined;
  /** The practice journey the record names, known once it is read: where no live assistant may open. */
  practice: string | undefined;
  /** The record is being read: which journeys are practice ones is not known yet. */
  reading: boolean;
  steps: Step[];
  index: number;
  /** The overlay's request of the canvas (cards to keep in view, in what room), or none to take it back. */
  setFraming: (request: Framing | undefined) => void;
  /** The example assistant reply has been shown. */
  replied: boolean;
  setReplied: (replied: boolean) => void;
  /** The link's words in the rail while the tour is open to the person, else none. */
  railLabel: string | undefined;
  /** The link's words on You: always there once the record is read. */
  youLabel: string | undefined;
  /** A closing write did not land: this tab keeps the offer shut. */
  unsaved: boolean;
  /** Puts the continue card away until the person is next on Home: nothing is stored and nothing ends. */
  putAway: () => void;
  /** Opens the tour from a link: Home, with the welcome (or the continue form) shown. */
  take: () => void;
  /** Goes to step `index`: its view, its node, its sheet height. */
  arrive: (index: number) => void;
  /** Ends the run from the callout or the paused line: Home first, then the practice journey goes. */
  end: (how: "finish" | "skip") => void;
}

const TourContext = createContext<TourApi | undefined>(undefined);

/** The tour, or none outside the shell (a screen rendered alone in a test). */
export function useTour(): TourApi | undefined {
  return useContext(TourContext);
}

/** How often a journey missing under a started record is looked for while it is waited for. */
const RETRY_MS = 1500;

/** Tells the machine whether the practice journey is there, as the app reads it. */
function RunFeed({ run, send, waiting }: { run: string; send: (event: TourEvent) => void; waiting: boolean }) {
  const status = useJourney(run).status;
  const retry = useRetryJourney();
  // While a missing journey is waited for, it is looked for again.
  useEffect(() => {
    if (!waiting) {
      return undefined;
    }
    const timer = setInterval(() => { retry(run); }, RETRY_MS);
    return () => { clearInterval(timer); };
  }, [waiting, retry, run]);
  useEffect(() => {
    if (status === "ready" || status === "missing") {
      send({ type: "journey", id: run, exists: status === "ready" });
    }
  }, [send, run, status]);
  return null;
}

/** The steps of the run, where the person is among them, and the way to a step. */
function useRunSteps(run: string | undefined, confirming: boolean) {
  const session = useSession();
  const { search } = useLocation();
  const navigate = useNavigate();
  const steps = useMemo(() => stepsFor(assistantOffered(session)), [session]);
  const [cursor, setCursor] = useDraft<number>(`tour:${run ?? ""}`);
  const [replied, setReplied] = useDraft<boolean>(`tour-replied:${run ?? ""}`);
  const arrive = useCallback(
    (to: number) => {
      const step = steps[to];
      if (run === undefined || step === undefined) {
        return;
      }
      const params = new URLSearchParams(search);
      if (step.frontier === true) {
        params.set("frontier", "1");
      } else if (step.frontier === false) {
        params.delete("frontier");
      }
      void navigate(journeyPath({ journey: run, node: step.node }, step.screen, params.toString()));
      setCursor(to);
    },
    [steps, run, search, navigate, setCursor],
  );
  // A tour just made opens on its first step.
  const arriveRef = useRef(arrive);
  arriveRef.current = arrive;
  useEffect(() => {
    if (confirming) {
      arriveRef.current(0);
    }
  }, [confirming]);
  const forget = useCallback(() => {
    setCursor(undefined);
    setReplied(undefined);
  }, [setCursor, setReplied]);
  const reply = useCallback((value: boolean) => { setReplied(value ? true : undefined); }, [setReplied]);
  return { steps, index: clampCursor(cursor, steps), arrive, replied: replied === true, forget, setReplied: reply };
}

/** What stands over the workspace (model.ts), less the continue card while the person has put it away for now (Escape): nothing is stored, the rail still offers the way back, and the card returns on the next visit to Home. */
function useSurface(input: SurfaceInput): [Surface, () => void, () => void] {
  const [declined, setDeclined] = useState(false);
  const { home } = input;
  useEffect(() => {
    if (!home) {
      setDeclined(false);
    }
  }, [home]);
  const putAway = useCallback(() => { setDeclined(true); }, []);
  const bringBack = useCallback(() => { setDeclined(false); }, []);
  const shown = surfaceOf(input);
  return [shown === "continue" && declined ? "none" : shown, putAway, bringBack];
}

/** Tells the machine whether the location is under the practice journey, and reads a record that could not be read again on the next visit to Home. */
function useReadAgain(unreadable: boolean, home: boolean, send: (event: TourEvent) => void, under: boolean): void {
  useEffect(() => {
    send({ type: "where", under });
  }, [send, under]);
  const wasHome = useRef(home);
  useEffect(() => {
    if (unreadable && home && !wasHome.current) {
      send({ type: "reload" });
    }
    wasHome.current = home;
  }, [send, unreadable, home]);
}

export function TourProvider({ nameSettled, children }: { nameSettled: boolean; children: ReactNode }) {
  const session = useSession();
  const ports = useMemo(() => tourPorts(session), [session]);
  const actor = useActorRef(tourMachine, { input: { ports } });
  const snapshot = useSelector(actor, (each) => each);
  const send = useCallback((event: TourEvent) => { actor.send(event); }, [actor]);
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const { run, tour: record, failure, invited, unsaved } = snapshot.context;
  const home = locate(pathname).home;
  const under = underJourney(pathname, run);
  useReadAgain(snapshot.matches("unreadable"), home, send, under);
  const { steps, index, arrive, replied, setReplied, forget } = useRunSteps(run, snapshot.matches({ running: "confirming" }));
  const [framing, setFraming] = useState<Framing | undefined>(undefined);
  const phase = phaseOf(snapshot);
  const offerable = record?.state === "none" || record?.state === "started";
  const [surface, putAway, bringBack] = useSurface({ phase, failure, offerable, invited, unsaved, run, pathname, home, nameSettled });
  const end = useCallback(
    (how: "finish" | "skip") => {
      forget();
      send({ type: how });
      void navigate(HOME_PATH);
    },
    [send, navigate, forget],
  );
  const take = useCallback(() => {
    bringBack();
    void navigate(HOME_PATH);
    if (actor.getSnapshot().matches("idle")) {
      send({ type: "invite" });
    }
  }, [actor, send, navigate, bringBack]);
  const open = eligible(phase, offerable, unsaved);
  const running = phase === "running";
  const api = useMemo<TourApi>(
    () => ({
      snapshot,
      send,
      surface,
      phase,
      record,
      run,
      practice: run ?? record?.practice_journey ?? undefined,
      reading: snapshot.matches("reading"),
      steps,
      index,
      setFraming,
      putAway,
      replied,
      setReplied,
      railLabel: !under && open && surface !== "welcome" && surface !== "continue" ? (running ? "Continue the tour" : "Take the tour") : undefined,
      youLabel: running ? "Continue the tour" : phase === "idle" ? (offerable ? "Take the tour" : "Take it again") : undefined,
      unsaved,
      take,
      arrive,
      end,
    }),
    [snapshot, send, surface, phase, record, run, steps, index, replied, setReplied, setFraming, putAway, under, open, running, offerable, unsaved, take, arrive, end],
  );
  return (
    <TourContext value={api}>
      <FramingContext value={framing}>
        {run === undefined ? null : <RunFeed run={run} send={send} waiting={snapshot.matches({ checking: "pending" })} />}
        {children}
      </FramingContext>
    </TourContext>
  );
}

/** Escape closes a card that has no target: Not now on the welcome (and on the continue card, which stays one visit away), Close on the end card. */
function useEscape(onEscape: () => void): void {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented && !typing(event.target) && !overlayOpen()) {
        onEscape();
      }
    };
    globalThis.addEventListener("keydown", onKey);
    return () => {
      globalThis.removeEventListener("keydown", onKey);
    };
  }, [onEscape]);
}

function Card({ title, label = "Tour of Cairn", testId, children }: { title: string; label?: string; testId: string; children: ReactNode }) {
  return (
    <section className="panel stack tour-card" aria-label={title} data-testid={testId}>
      <span className="label">{label}</span>
      <h2 className="tour-title">{title}</h2>
      {children}
    </section>
  );
}

function Welcome({ api }: { api: TourApi }) {
  const { send, record } = api;
  const waiting = record?.state === "started";
  // Not now on an offer the person asked for after the tour ended changes nothing stored.
  const decline = useCallback(() => {
    send(record?.state === "none" || record?.state === "started" ? { type: "dismiss" } : { type: "uninvite" });
  }, [send, record]);
  const skip = useCallback(() => { api.end("skip"); }, [api]);
  // Escape declines for now, on the continue card as on the welcome: only the named button skips, which deletes the practice journey.
  useEscape(waiting ? api.putAway : decline);
  if (waiting && api.run !== undefined) {
    return (
      <Card title="Your tour is waiting" testId="tour-continue">
        <p>You started the tour; its practice journey is still here.</p>
        <div className="row">
          <Button primary data-testid="tour-resume" onClick={() => { api.arrive(api.index); }}>Continue the tour</Button>
          <Button data-testid="tour-skip" onClick={skip}>Skip the tour</Button>
        </div>
      </Card>
    );
  }
  return (
    <Card title="See what Cairn works out for you" testId="tour-welcome">
      <p>
        A route is a reusable process. A journey is one real run of it: its answers, its progress, its dates. Cairn works out what is ready, what is blocked, what applies and when things are due, so people and their agents share one picture of what to do next.
      </p>
      <p>Try it on a small practice journey. It is made for you now and deleted when the tour ends; your own journeys do not change.</p>
      <div className="row">
        <Button primary data-testid="tour-start" onClick={() => { send({ type: "start" }); }}>Start the tour</Button>
        <Button data-testid="tour-not-now" onClick={decline}>Not now</Button>
      </div>
      <p className="muted small">Tab moves between the guidance and the step's control.</p>
    </Card>
  );
}

function Failure({ api, cleanup }: { api: TourApi; cleanup: boolean }) {
  const { send } = api;
  const close = useCallback(() => { send({ type: "close" }); }, [send]);
  useEscape(close);
  return (
    <Card title={cleanup ? "The practice journey could not be removed" : "The tour could not start"} testId="tour-failed">
      <p role="alert">{cleanup ? "Your own journeys are not affected. Try again, or leave it and come back to the tour later." : "Nothing was changed. Try again, or take the tour later from the left."}</p>
      <div className="row">
        <Button primary data-testid="tour-retry" onClick={() => { send({ type: "retry" }); }}>{cleanup ? "Retry" : "Try again"}</Button>
        <Button data-testid="tour-close" onClick={close}>Close</Button>
      </div>
    </Card>
  );
}

function End({ api }: { api: TourApi }) {
  const { send } = api;
  const close = useCallback(() => { send({ type: "close" }); }, [send]);
  useEscape(close);
  return (
    <Card title="That's the core" testId="tour-end">
      <p>
        You answered a decision and finished work; Cairn worked out what became ready and what is due when. Every journey has a Graph, a List, a Timeline, a Summary and a Walkthrough of its open decisions. Start your own journey from a route or from nothing.
      </p>
      <p>
        The practice journey is gone; you can take the tour again from <Link to={pathTo("me")}>You</Link>.
      </p>
      <div className="row">
        <Link className="button primary" to="/journeys/new" data-testid="tour-new-journey" onClick={close}>New journey</Link>
        <Button data-testid="tour-close" onClick={close}>Close</Button>
      </div>
    </Card>
  );
}

function Paused({ api }: { api: TourApi }) {
  return (
    <div className="tour-paused row" role="status" data-testid="tour-paused">
      <span className="label">Tour paused</span>
      <Button ghost data-testid="tour-resume" onClick={() => { api.arrive(api.index); }}>Continue</Button>
      <Button ghost data-testid="tour-skip" onClick={() => { api.end("skip"); }}>Skip the tour</Button>
    </div>
  );
}

/** Whatever the tour puts above the workspace now. */
export function Tour() {
  const api = useTour();
  if (api === undefined) {
    return null;
  }
  switch (api.surface) {
    case "none":
      return null;
    case "welcome":
    case "continue":
      return <Welcome api={api} />;
    case "start-failed":
      return <Failure api={api} cleanup={false} />;
    case "cleanup-failed":
      return <Failure api={api} cleanup />;
    case "end":
      return <End api={api} />;
    case "paused":
      return <Paused api={api} />;
    case "overlay":
      return api.run === undefined ? null : <Overlay api={api} run={api.run} />;
  }
}
