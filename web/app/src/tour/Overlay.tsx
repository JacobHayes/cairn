// PRD: C20.
// The tour's step as a spotlight over the live screen. A step that asks for an action has two
// states. While the action waits (the spotlight) the screen is dimmed, the complete action area is
// cut out, and a callout beside it says what to do. When the derived state shows the action done
// (the reveal) the dimming lifts, or widens to take in the change when it grows out of the action
// area as one box, and the callout moves beside what changed and says what Cairn worked out; the
// person moves on with Next, which dims the next step's target. Nothing moves on a timer.
//
// What a step points at is steps.ts (selectors over the live surface); the geometry is track.ts;
// the keyboard is focus.ts; this file measures once per animation frame, draws, and asks the canvas
// (presentation.ts) and the scrollers to leave the callout room.
import { useCallback, useEffect, useId, useMemo, useRef, useState, type CSSProperties, type RefObject } from "react";
import { useLocation } from "react-router";

import { shiftInto } from "../canvas/gestures.ts";
import { useJourney } from "../data/react.ts";
import { viewStateFrom } from "../journeys/view.ts";
import { PHONE_WIDTH, SHEET, SHORT, useFrameActions, useFrameState, useMedia, type FrameActions, type FrameState } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { place, type Rect, type Side } from "../ui/place.ts";
import { useFocusKeeper } from "./focus.ts";
import type { Framing } from "./presentation.ts";
import { factsOf, stepDone, type Facts, type Step, type Where } from "./steps.ts";
import {
  CALLOUT_GAP,
  HOLE_PAD,
  SETTLE_FRAMES,
  alignFor,
  chooseCallout,
  DOCK_MARGIN,
  dockRect,
  dockWidth,
  diagnose,
  freeRoom,
  grow,
  isNarrow,
  match,
  rectOf,
  sameRect,
  scrimOf,
  scrollerOf,
  strip,
  union,
  viewportRect,
  visibleRects,
  type Absence,
  type Callout,
  type Size,
} from "./track.ts";
import type { TourApi } from "./Tour.tsx";
import { locate } from "./where.ts";
import "./overlay.css";

type Phase = "spotlight" | "reveal";

/** How long a target may be missing before the callout is shown without it (ms): the view, the pane or the layout is on its way. */
const ALONE_MS = 600;

/** How many frames after a state begins the scrollers are held to it, so a person who scrolls later is left alone. */
const HOLD_FRAMES = 90;

/** The space the callout keeps from the window's edge (px). */
const MARGIN = 12;

/** What one animation frame read of the page. */
interface Snap {
  /** What the spotlight cuts out (padded), and what the callout sits beside; none when no match is on the screen. */
  hole: Rect | undefined;
  anchor: Rect | undefined;
  /** What the step's sentence names, as far as it shows: the callout never covers these. */
  named: Rect[];
  /** Matches exist but are scrolled out of their scroller. */
  scrolledOut: boolean;
  size: Size;
  /** What its size would be docked across the foot. */
  docked: Size;
  viewport: Size;
  /** The window's foot, or the top of the sheet where one lies over it. */
  floor: number;
  /** Why there is no anchor, once there is none. */
  absence: Absence | undefined;
}

const sameSize = (a: Size, b: Size): boolean => a.width === b.width && a.height === b.height;

const sameSnap = (a: Snap | undefined, b: Snap): boolean =>
  a !== undefined && sameRect(a.hole, b.hole) && sameRect(a.anchor, b.anchor) && a.named.length === b.named.length && a.named.every((each, at) => sameRect(each, b.named[at])) && a.scrolledOut === b.scrolledOut && sameSize(a.size, b.size) && sameSize(a.docked, b.docked) && sameSize(a.viewport, b.viewport) && a.floor === b.floor && a.absence === b.absence;

/** What the overlay reads for a state: the selectors of the hole and the anchor, and what is asked of the canvas. */
interface Targets {
  key: string;
  step: Step;
  phase: Phase;
  hole: string[];
  anchor: string[];
  /** What the sentence is about, for the callout to leave uncovered. */
  names: string[];
  side: Side;
  frame: readonly string[];
  /** The anchor is a card the canvas has yet to frame, so its place is not known until it has. */
  onCanvas: boolean;
}

const isCard = (selector: string): boolean => selector.startsWith('[data-testid="node-card"]');

/** In a window too short to hold the callout above or below a card and leave the card room, the guidance sits beside it. */
function sideIn(side: Side, short: boolean): Side {
  return short && (side === "top" || side === "bottom") ? "left" : side;
}

/** The state the step is in: spotlight while its action waits, reveal once done (a step with no action has the spotlight only). */
function targetsOf(step: Step, facts: Facts, where: Where, short: boolean): Targets {
  const done = stepDone(step, facts, where);
  const phase: Phase = done && step.reveal !== "none" ? "reveal" : "spotlight";
  const area = step.area(facts, where);
  const anchor = phase === "spotlight" ? area : (step.anchor?.(facts, where) ?? []);
  return {
    key: [step.id, phase, ...anchor].join("|"),
    step,
    phase,
    hole: phase === "spotlight" ? area : step.reveal === "widen" ? [...area, ...anchor] : [],
    anchor,
    names: step.names?.(facts, phase) ?? [],
    side: sideIn(phase === "spotlight" ? step.side : (step.anchorSide ?? step.side), short),
    frame: phase === "spotlight" ? (step.frame ?? []) : (step.revealFrame?.(facts) ?? []),
    onCanvas: anchor.length > 0 && anchor.every(isCard),
  };
}

/** The text a callout shows for an absent target. */
const ABSENT_WORDS: Record<Absence, string | undefined> = {
  loading: undefined,
  closed: "The inspector is lowered. Raise it to continue.",
  missing: "This step's control is not on the screen.",
};

// ---- measuring ----------------------------------------------------------------------------------

/** What the frame loop reads from the latest render. */
interface Latest {
  targets: Targets | undefined;
  facts: Facts | undefined;
  replied: boolean;
  setFraming: (request: Framing | undefined) => void;
  frameState: FrameState | undefined;
}

/** One state's frame loop, and what it has already done for the state. */
interface Run {
  last: Snap | undefined;
  held: number;
  frames: number;
  absentSince: number | undefined;
  framedFor: string | undefined;
  /** The callout had to dock for a state that was framed for one beside its target, and the canvas was asked again. */
  redocked: boolean;
  /** The canvas has framed what it was last asked to (or was asked nothing). */
  framed: boolean;
  opened: boolean;
  reopened: boolean;
}

/** The window's foot: the top of the sheet where one lies over it (below 1100px). */
function floorOf(viewport: Rect): number {
  const sheet = document.querySelector<HTMLElement>("aside.inspector");
  return globalThis.matchMedia(SHEET).matches && sheet !== null && !sheet.hidden && sheet.dataset["folded"] !== "true" ? rectOf(sheet).top : viewport.height;
}

/** The hole of a target that is in the sheet takes in the sheet's handle, so the person can resize the sheet while the rest stays dim. */
function withHandle(rect: Rect, elements: readonly HTMLElement[]): Rect {
  const handle = document.querySelector<HTMLElement>(".sheet-handle");
  const inSheet = elements.some((each) => each.closest("aside.inspector") !== null);
  return inSheet && globalThis.matchMedia(SHEET).matches && handle !== null && handle.checkVisibility() ? (union([rect, rectOf(handle)]) ?? rect) : rect;
}

/** What the page shows of a state's targets now. */
function readSnap(now: Targets, element: HTMLElement, absentFor: number): { snap: Snap; anchored: ReturnType<typeof match> } {
  const holes = match(now.hole);
  const anchored = now.phase === "spotlight" ? holes : match(now.anchor);
  const viewport = viewportRect();
  const peeking = globalThis.matchMedia(SHEET).matches && document.querySelector<HTMLElement>(".frame")?.dataset["sheet"] === "peek";
  return {
    anchored,
    snap: {
      hole: holes.rect === undefined ? undefined : grow(withHandle(holes.rect, holes.elements), HOLE_PAD),
      anchor: anchored.rect === undefined ? undefined : grow(anchored.rect, now.phase === "spotlight" ? HOLE_PAD : 6),
      named: visibleRects(now.names),
      scrolledOut: holes.scrolledOut || anchored.scrolledOut,
      ...sizesOf(element, viewport),
      viewport: { width: viewport.width, height: viewport.height },
      floor: floorOf(viewport),
      absence: anchored.rect === undefined ? diagnose({ loaded: true, closed: peeking && now.step.pane === true, absentFor }) : undefined,
    },
  };
}

/** Opens what the step is about (a disclosure) once it is on the screen, and the assistant's pane when a revisit finds it shut. */
function prepare(now: Targets, latest: Latest, run: Run): void {
  if (!run.opened && now.step.opens !== undefined) {
    const found = now.step.opens.flatMap((selector) => [...document.querySelectorAll<HTMLDetailsElement>(selector)]);
    found.forEach((each) => { each.open = true; });
    run.opened = found.length === now.step.opens.length;
  }
  // The pane is mounted only while open, so once the reply has been shown a revisit finds the tab alone.
  if (!run.reopened && now.step.assistantPane !== undefined && latest.replied && document.querySelector(now.step.assistantPane) === null) {
    run.reopened = true;
    latest.frameState?.openAssistant?.();
  }
}

let framings = 0;

/** What to ask of the canvas for this state, or nothing off the graph; `docked` when the callout is across the foot of the window and not beside a target. */
function framingFor(now: Targets, snap: Snap, docked: boolean, applied: () => void): Framing | undefined {
  const canvas = document.querySelector<HTMLElement>('[data-testid="canvas"]');
  if (now.step.screen !== "graph" || canvas === null || now.frame.length === 0) {
    return undefined;
  }
  const { size, docked: stretched, viewport, floor, anchor } = snap;
  // The canvas above the sheet that lies over its foot: the room the cards and the callout share.
  const whole = rectOf(canvas);
  const region = { ...whole, height: Math.max(0, Math.min(whole.top + whole.height, floor) - whole.top) };
  framings += 1;
  const room = (taken: Rect | undefined): Framing => ({ id: framings, cards: now.frame, room: taken === undefined ? region : freeRoom(region, taken), applied });
  if (docked) {
    const dock = dockRect(stretched, viewport, floor);
    // The whole width of the foot, and the ring's pad above it: a callout capped narrower than a wide window still leaves no room beside it worth framing in.
    return room({ left: region.left, width: region.width, top: dock.top - MARGIN - HOLE_PAD, height: dock.height + 2 * MARGIN + HOLE_PAD });
  }
  // Where the callout will be: along the near side of the canvas for a card it has yet to sit beside, against the target for one that is on the screen.
  if (now.onCanvas) {
    return room(strip(region, now.side, size, CALLOUT_GAP + HOLE_PAD + MARGIN));
  }
  if (anchor === undefined) {
    return room(undefined);
  }
  const at = place(anchor, size, { left: 0, top: 0, width: viewport.width, height: floor }, { side: now.side, align: alignFor(anchor, now.side), gap: CALLOUT_GAP, margin: MARGIN });
  return room({ left: at.left - MARGIN, top: at.top - MARGIN, width: size.width + 2 * MARGIN, height: size.height + 2 * MARGIN });
}

/** Asks the canvas to frame (or takes the request back); until it has, the state is not judged. */
function askCanvas(run: Run, setFraming: (request: Framing | undefined) => void, request: Framing | undefined): void {
  run.framed = request === undefined;
  setFraming(request);
}

/** Scrolls the element's scroller, if it has one, so the element, with what the sentence names beside it in that scroller, shows clear of the callout (above or below it, or the one docked across the foot). */
function keepInRoom(element: HTMLElement | undefined, now: Targets, snap: Snap): void {
  const scroller = element === undefined ? undefined : scrollerOf(element);
  if (element === undefined || scroller === undefined) {
    return;
  }
  const box = rectOf(scroller);
  const docked = isNarrow(snap.viewport);
  const reserve = snap.size.height + CALLOUT_GAP + HOLE_PAD + MARGIN;
  const dockTop = snap.floor - DOCK_MARGIN - snap.docked.height - CALLOUT_GAP;
  const [low, high] = [box.top + (!docked && now.side === "top" ? reserve : 0), Math.min(box.top + box.height - (!docked && now.side === "bottom" ? reserve : 0), docked ? dockTop : Infinity)];
  const group = [element, ...now.names.flatMap((each) => [...document.querySelectorAll<HTMLElement>(each)])].filter((each) => scrollerOf(each) === scroller).map(rectOf);
  const target = union(group) ?? rectOf(element);
  const move = shiftInto(target.top - HOLE_PAD, target.height + 2 * HOLE_PAD, low, high);
  if (Math.abs(move) >= 1) {
    scroller.scrollTop -= move;
  }
}

/** The callout's size at its own width, and at the width it has docked: where it fits beside a target is a question about the first, where it sits across the foot about the second. */
function sizesOf(element: HTMLElement, viewport: Size): { size: Size; docked: Size } {
  const [stretched, side] = [element.style.width, element.dataset.side];
  const read = (width: string, as: string): Size => {
    // The side is part of what sets the size (a docked callout trims its body in a short window), so each reading names its own.
    element.style.width = width;
    element.dataset.side = as;
    return { width: element.offsetWidth, height: element.offsetHeight };
  };
  const [size, docked] = [read("", "top"), read(`${String(dockWidth(viewport))}px`, "dock")];
  element.style.width = stretched;
  if (side === undefined) {
    delete element.dataset.side;
  } else {
    element.dataset.side = side;
  }
  return { size, docked };
}

/** The state last drawn: what the callout and the scrim show. */
interface Shown {
  key: string;
  step: string;
  phase: Phase;
}

/**
 * Once per animation frame, the targets are read; a change (a pan, a scroll, a sheet's drag, a
 * menu opening) re-places the callout and the hole. A state is drawn once its targets have held
 * still for a few frames (or are missing and not coming), and the canvas and the scrollers are
 * asked to leave the callout room before that.
 */
function useMeasure(key: string | undefined, latest: RefObject<Latest>, callout: RefObject<HTMLElement | null>, shown: RefObject<Shown | undefined>): { snap: Snap | undefined; shown: Shown | undefined; restart: () => void } {
  const [snap, setSnap] = useState<Snap | undefined>(undefined);
  // A restart draws the state afresh: the canvas is asked to frame the target again and the scrollers to bring it into view.
  const [again, setAgain] = useState(0);
  const [drawn, setDrawn] = useState<Shown | undefined>(undefined);
  useEffect(() => {
    if (key === undefined) {
      return undefined;
    }
    let frame = 0;
    const run: Run = { last: undefined, held: 0, frames: 0, absentSince: undefined, framedFor: undefined, redocked: false, framed: true, opened: false, reopened: false };
    const tick = () => {
      frame = requestAnimationFrame(tick);
      const { targets: now, facts, setFraming } = latest.current;
      const element = callout.current;
      if (now?.key !== key || element === null || facts === undefined) {
        return;
      }
      run.frames += 1;
      const { snap: next, anchored } = readSnap(now, element, run.absentSince === undefined ? 0 : performance.now() - run.absentSince);
      run.absentSince = anchored.rect === undefined ? (run.absentSince ?? performance.now()) : undefined;
      if (sameSnap(run.last, next)) {
        run.held += 1;
      } else {
        [run.last, run.held] = [next, 0];
        setSnap(next);
      }
      prepare(now, latest.current, run);
      const absentFor = run.absentSince === undefined ? 0 : performance.now() - run.absentSince;
      // The canvas is asked to keep the cards in view in the room the callout leaves, once the callout's size is known, and again when the window changes.
      const framedFor = `${now.key}@${String(next.viewport.width)}x${String(next.viewport.height)}/${String(Math.round(next.floor))}`;
      // A new state is asked for at once; a new window, or a sheet that has moved, once it holds still (a drag is not followed).
      if (run.framedFor !== framedFor && (run.framedFor?.startsWith(`${now.key}@`) !== true || run.held >= SETTLE_FRAMES) && next.size.width > 0 && (now.step.screen !== "graph" || now.frame.length === 0 || document.querySelector('[data-testid="canvas"]') !== null) && (now.onCanvas || next.anchor !== undefined || absentFor > ALONE_MS)) {
        run.framedFor = framedFor;
        askCanvas(run, setFraming, framingFor(now, next, isNarrow(next.viewport), () => { [run.framed, run.held] = [true, -3]; }));
      }
      // Beside its target the callout would cover what the sentence names, so it docks: the canvas keeps its cards clear of the foot then.
      if (!run.redocked && run.framed && run.held >= SETTLE_FRAMES && next.anchor !== undefined && placeCallout(next, now).side === "dock" && !isNarrow(next.viewport)) {
        run.redocked = true;
        askCanvas(run, setFraming, framingFor(now, next, true, () => { [run.framed, run.held] = [true, -3]; }));
      }
      if (shown.current?.key !== now.key && run.frames < HOLD_FRAMES) {
        keepInRoom(anchored.elements[0], now, next);
      }
      const settled = anchored.rect !== undefined && run.held >= SETTLE_FRAMES;
      const alone = anchored.rect === undefined && (absentFor > ALONE_MS || next.absence === "closed" || next.absence === "missing");
      if ((settled || alone) && shown.current?.key !== now.key) {
        setDrawn({ key: now.key, step: now.step.id, phase: now.phase });
      }
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
    };
    // The loop reads what changes from `latest`; it restarts only for a new state.
  }, [key, latest, callout, shown, again]);
  return { snap, shown: drawn, restart: useCallback(() => { setDrawn(undefined); setAgain((count) => count + 1); }, []) };
}

// ---- what a state does for the person -------------------------------------------------------------

/** A step's arrival puts the reader at its title; a reveal keeps the focus where the action left it unless that is gone. */
function useFocusOnShown(shown: Shown | undefined, title: RefObject<HTMLElement | null>): void {
  const arrivedAt = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (shown === undefined) {
      return;
    }
    if (arrivedAt.current !== shown.step) {
      arrivedAt.current = shown.step;
      title.current?.focus();
      return;
    }
    const active = document.activeElement;
    if (shown.phase === "reveal" && (!(active instanceof HTMLElement) || active === document.body || !active.isConnected || active.closest("[inert]") !== null || active.matches(":disabled"))) {
      title.current?.focus();
    }
  }, [shown, title]);
}

/** On reaching the reveal: the sheet takes the height that shows the change, and a menu the action left open closes. */
function useRevealEffects(shown: Shown | undefined, steps: readonly Step[], sheet: boolean, actions: FrameActions | undefined): void {
  // The step whose state was drawn, which is not yet the current one while the callout is on its way to it.
  const step = steps.find((each) => each.id === shown?.step);
  const [phase, id] = [shown?.phase, shown?.step];
  useEffect(() => {
    if (phase !== "reveal" || step === undefined) {
      return;
    }
    if (step.revealSheet !== undefined && sheet) {
      actions?.snap(step.revealSheet);
    }
    if (step.revealClose !== undefined) {
      document.querySelector<HTMLElement>(step.revealClose)?.click();
    }
    // Once per arrival in the reveal, whatever else the render changes.
  }, [phase, id]);
}

/** A step that needs the inspector's pane raises the sheet when a state of it begins (a first arrival, a Back, a reload), so the action is in view without a tap. */
function useArrivalSheet(targets: Targets | undefined, sheet: boolean, actions: FrameActions | undefined): void {
  const key = targets?.key;
  useEffect(() => {
    if (targets?.phase === "spotlight" && targets.step.pane === true && sheet) {
      actions?.snap("half");
    }
    // Once per state, whatever else the render changes.
  }, [key]);
}

/** True for a moment after each state is drawn: the callout glides to its new place then, and is placed at once while a pan or a scroll drags its anchor along. */
function useGliding(key: string | undefined): boolean {
  const [gliding, setGliding] = useState(false);
  useEffect(() => {
    if (key === undefined) {
      return undefined;
    }
    setGliding(true);
    const timer = setTimeout(() => { setGliding(false); }, 260);
    return () => { clearTimeout(timer); };
  }, [key]);
  return gliding;
}

// ---- drawing ------------------------------------------------------------------------------------

/** The callout's place from what was read. */
function placeCallout(snap: Snap, targets: Targets): Callout {
  return chooseCallout({ anchor: snap.anchor, avoid: [...(snap.anchor === undefined ? [] : [snap.anchor]), ...snap.named], size: snap.size, docked: snap.docked, viewport: snap.viewport, floor: snap.floor, side: targets.side });
}

/** The dimming around the hole, and the ring around the hole. */
function Scrim({ snap, lifted, gliding }: { snap: Snap | undefined; lifted: boolean; gliding: boolean }) {
  const hole = snap?.hole;
  return (
    <div className="tour-scrim-layer" data-testid="tour-scrim" data-lifted={lifted} data-gliding={gliding} aria-hidden="true">
      {hole === undefined || snap === undefined
        ? null
        : scrimOf(hole, snap.viewport).map((rect, each) => <div key={each} className="tour-scrim" style={{ left: rect.left, top: rect.top, width: rect.width, height: rect.height }} />)}
      {hole === undefined ? null : <div className="tour-ring" data-testid="tour-ring" style={{ left: hole.left, top: hole.top, width: hole.width, height: hole.height }} />}
    </div>
  );
}

interface CalloutViewProps {
  refs: { callout: RefObject<HTMLElement | null>; title: RefObject<HTMLHeadingElement | null> };
  api: TourApi;
  step: Step;
  done: boolean;
  words: string;
  absence: Absence | undefined;
  scrolledOut: boolean;
  at: Callout | undefined;
  ready: boolean;
  revealed: boolean;
  gliding: boolean;
  /** The callout is the app's open layer: the app's keys stand down and the focus is kept. */
  blocking: boolean;
  onShowAgain: () => void;
}

/** The callout: where the step is, what to do or what Cairn worked out, and the way on. */
function CalloutView({ refs, api, step, done, words, absence, scrolledOut, at, ready, revealed, gliding, blocking, onShowAgain }: CalloutViewProps) {
  const { steps, index } = api;
  const [titleId, bodyId] = [useId(), useId()];
  const last = index === steps.length - 1;
  const style: CSSProperties | undefined =
    at === undefined ? undefined : { left: at.left, ...(at.bottomGap === undefined ? { top: at.top } : { bottom: at.bottomGap }), ...(at.width === undefined ? {} : { width: at.width }), ...(at.notchAt === undefined ? {} : { "--notch-at": `${String(at.notchAt)}px` }) };
  const next = () => {
    if (done) {
      if (last) {
        api.end("finish");
      } else {
        api.arrive(index + 1);
      }
    }
  };
  return (
    <section
      ref={refs.callout}
      className="tour-callout"
      role="dialog"
      aria-modal="false"
      aria-labelledby={titleId}
      aria-describedby={bodyId}
      data-testid="tour-callout"
      data-step={step.id}
      data-phase={revealed ? "reveal" : "spotlight"}
      data-done={done}
      data-side={at?.side ?? "dock"}
      data-ready={ready}
      data-gliding={gliding}
      data-overhang=""
      {...(blocking ? { "data-popover": "" } : {})}
      style={style}
    >
      <span className="label" data-testid="tour-label">{`Tour · step ${String(index + 1)} of ${String(steps.length)} · ${step.where}`}</span>
      <h2 ref={refs.title} id={titleId} tabIndex={-1} className="tour-title" data-testid="tour-title">{step.title}</h2>
      <div className="tour-body" id={bodyId} role="status" data-testid="tour-body">
        <p>{words}</p>
        {scrolledOut ? <p className="muted">Scroll to see it.</p> : null}
      </div>
      {absence === "closed" || absence === "missing" ? (
        <div className="tour-actions">
          <Button data-testid="tour-show" onClick={onShowAgain}>{absence === "closed" ? "Raise the inspector" : "Show the control"}</Button>
        </div>
      ) : null}
      <div className="tour-actions">
        {index === 0 ? null : <Button data-testid="tour-back" onClick={() => { api.arrive(index - 1); }}>Back</Button>}
        <Button primary held={done ? undefined : step.waits} data-testid="tour-next" onClick={next}>
          {last ? "Finish the tour" : "Next"}
        </Button>
        <Button ghost data-testid="tour-skip" onClick={() => { api.end("skip"); }}>Skip the tour</Button>
      </div>
      {done ? null : <p className="tour-waits small muted" data-testid="tour-waits">{step.waits}</p>}
    </section>
  );
}

export function Overlay({ api, run }: { api: TourApi; run: string }) {
  const { steps, index, setFraming } = api;
  const { pathname, search } = useLocation();
  const [phone, short, sheet] = [useMedia(PHONE_WIDTH), useMedia(SHORT), useMedia(SHEET)];
  const actions = useFrameActions();
  const frameState = useFrameState();
  const view = useJourney(run);
  const step = steps[index];
  const facts = view.status === "ready" ? factsOf(view) : undefined;
  const where: Where = { selected: locate(pathname).node, frontier: viewStateFrom(new URLSearchParams(search)).frontier, replied: api.replied };
  const targets = step === undefined || facts === undefined ? undefined : targetsOf(step, facts, where, short);
  const done = step !== undefined && facts !== undefined && stepDone(step, facts, where);
  const lifted = targets?.phase === "reveal" && step?.reveal === "lift";
  const callout = useRef<HTMLElement>(null);
  const title = useRef<HTMLHeadingElement>(null);
  const latest = useRef<Latest>({ targets, facts, replied: api.replied, setFraming, frameState });
  latest.current = { targets, facts, replied: api.replied, setFraming, frameState };
  const shownRef = useRef<Shown | undefined>(undefined);
  const { snap, shown, restart } = useMeasure(targets?.key, latest, callout, shownRef);
  shownRef.current = shown;
  const holeNow = useRef<Rect | undefined>(snap?.hole);
  holeNow.current = snap?.hole;
  // Leaving the tour (or the practice journey) takes its request of the canvas away.
  useEffect(() => () => { setFraming(undefined); }, [setFraming]);
  useFocusOnShown(shown, title);
  useRevealEffects(shown, steps, sheet, actions);
  useArrivalSheet(targets, sheet, actions);
  const gliding = useGliding(shown?.key);
  useFocusKeeper({ callout, title, hole: useCallback(() => holeNow.current, []), enforcing: !lifted && shown !== undefined && snap?.hole !== undefined });

  const at = useMemo(() => (snap === undefined || targets === undefined ? undefined : placeCallout(snap, targets)), [snap, targets]);
  if (step === undefined) {
    return null;
  }
  const absence = snap?.absence;
  const current = shown?.step === step.id;
  // The text follows the state last drawn, so a callout still on its way to the change does not already say it.
  const revealed = current && shown.phase === "reveal";
  const words = ABSENT_WORDS[absence ?? "loading"] ?? (facts === undefined ? "Reading the practice journey..." : revealed ? step.result?.(facts) : undefined) ?? (facts === undefined ? "" : step.instruction(facts, { phone, short }));
  const showAgain = () => {
    if (step.assistantPane !== undefined) {
      frameState?.openAssistant?.();
    } else if (absence === "closed") {
      actions?.snap("half");
    } else {
      restart();
    }
  };
  return (
    <>
      <Scrim snap={snap} lifted={lifted || !current} gliding={gliding} />
      <CalloutView
        refs={{ callout, title }}
        api={api}
        step={step}
        done={done}
        words={words}
        absence={absence}
        scrolledOut={snap?.scrolledOut === true}
        at={at}
        ready={current && at !== undefined}
        revealed={revealed}
        gliding={gliding}
        blocking={!lifted}
        onShowAgain={showAgain}
      />
    </>
  );
}
