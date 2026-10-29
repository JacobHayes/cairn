// PRD: C20.
// The tour's step as a spotlight over the live screen. A step that asks for an action has two
// states. While the action waits (the spotlight) the screen is dimmed, the action area is cut out,
// and a callout beside it says what to do. When the derived state shows the action done (the
// reveal) the dimming lifts, or widens to take in the change when it grows out of the action area,
// and the callout moves beside what changed and says what Cairn worked out; the person moves on
// with Next. Nothing moves on a timer.
//
// What a step points at is steps.ts (selectors over the live surface). The rectangles are read
// when something that could move them happens (the window or the callout resizes, the page
// scrolls or its markup changes), never on a clock, and the callout is placed once per reading.
import { useEffect, useId, useRef, useState, type CSSProperties, type RefObject } from "react";
import { useLocation } from "react-router";

import { useJourney } from "../data/react.ts";
import { viewStateFrom } from "../journeys/view.ts";
import { PHONE_WIDTH, SHEET, SHORT, useFrameActions, useFrameState, useMedia } from "../shell/frame.tsx";
import { Button } from "../ui/kit.tsx";
import { place, type Rect, type Side } from "../ui/place.ts";
import type { Framing } from "./presentation.ts";
import { factsOf, stepDone, type Facts, type Step, type Where } from "./steps.ts";
import type { TourApi } from "./Tour.tsx";
import { locate } from "./where.ts";
import "./overlay.css";

type Phase = "spotlight" | "reveal";

/** The space between a target and the hole's edge, between the hole and the callout, and between the callout and the window's edge (px). */
const HOLE_PAD = 8;
const GAP = 16;
const MARGIN = 12;

/** A window narrower than this docks the callout across the foot instead of beside its target (px). */
const NARROW = 720;

interface Size {
  width: number;
  height: number;
}

/** What the page shows now: where the targets are, the window, the foot of the room above the sheet, and the callout's size. */
interface Layout {
  hole: Rect | undefined;
  anchor: Rect | undefined;
  viewport: Size;
  floor: number;
  size: Size;
}

const sameRect = (a: Rect | undefined, b: Rect | undefined): boolean =>
  a === b || (a !== undefined && b !== undefined && Math.abs(a.left - b.left) < 0.5 && Math.abs(a.top - b.top) < 0.5 && Math.abs(a.width - b.width) < 0.5 && Math.abs(a.height - b.height) < 0.5);

const sameLayout = (a: Layout | undefined, b: Layout): boolean =>
  a !== undefined && sameRect(a.hole, b.hole) && sameRect(a.anchor, b.anchor) && a.viewport.width === b.viewport.width && a.viewport.height === b.viewport.height && a.floor === b.floor && a.size.width === b.size.width && a.size.height === b.size.height;

/** The rectangle of `element`, in the window's coordinates. */
function rectOf(element: Element): Rect {
  const { left, top, width, height } = element.getBoundingClientRect();
  return { left, top, width, height };
}

/** The drawn, not inert elements `selectors` match on the page now. */
function matches(selectors: readonly string[]): HTMLElement[] {
  return selectors
    .flatMap((selector) => [...document.querySelectorAll<HTMLElement>(selector)])
    .filter((each) => each.closest("[inert]") === null && each.checkVisibility({ visibilityProperty: true }) && each.offsetWidth > 0 && each.offsetHeight > 0);
}

/** The part of `element` that shows: its rectangle less what the window and every ancestor that clips its overflow hide; none when none of it does. */
function shownRect(element: Element): Rect | undefined {
  let [left, top, right, bottom] = [0, 0, document.documentElement.clientWidth, globalThis.innerHeight];
  const box = rectOf(element);
  [left, top, right, bottom] = [Math.max(left, box.left), Math.max(top, box.top), Math.min(right, box.left + box.width), Math.min(bottom, box.top + box.height)];
  for (let each = element.parentElement; each !== null && each !== document.body; each = each.parentElement) {
    const { overflowX, overflowY } = getComputedStyle(each);
    if (overflowX !== "visible" || overflowY !== "visible") {
      const clip = rectOf(each);
      [left, top, right, bottom] = [Math.max(left, clip.left), Math.max(top, clip.top), Math.min(right, clip.left + clip.width), Math.min(bottom, clip.top + clip.height)];
    }
  }
  return right > left && bottom > top ? { left, top, width: right - left, height: bottom - top } : undefined;
}

/** The smallest rectangle holding what shows of `elements`, grown by `pad`; none for none. */
function around(elements: readonly HTMLElement[], pad: number): Rect | undefined {
  const rects = elements.flatMap((each) => shownRect(each) ?? []);
  if (rects.length === 0) {
    return undefined;
  }
  const [left, top] = [Math.min(...rects.map((each) => each.left)) - pad, Math.min(...rects.map((each) => each.top)) - pad];
  return { left, top, width: Math.max(...rects.map((each) => each.left + each.width)) + pad - left, height: Math.max(...rects.map((each) => each.top + each.height)) + pad - top };
}

/** The window's foot, or the top of the sheet where one lies over it (below 1100px). */
function floorOf(): number {
  const sheet = document.querySelector<HTMLElement>("aside.inspector");
  return globalThis.matchMedia(SHEET).matches && sheet !== null && !sheet.hidden && sheet.dataset["folded"] !== "true" ? rectOf(sheet).top : globalThis.innerHeight;
}

/** Brings an element that sits in a scroller into its view; the canvas pans itself and is left alone. */
function scrollIn(element: HTMLElement): void {
  for (let each = element.parentElement; each !== null; each = each.parentElement) {
    const { overflowY } = getComputedStyle(each);
    if ((overflowY === "auto" || overflowY === "scroll") && each.scrollHeight > each.clientHeight) {
      element.scrollIntoView({ block: "nearest" });
      return;
    }
  }
}

/** What a state of a step points at, and what it does to the page once it is there. */
interface Targets {
  step: Step;
  phase: Phase;
  hole: string[];
  anchor: string[];
  side: Side;
}

/** The state the step is in: spotlight while its action waits, reveal once done (a step with no action has the spotlight only). */
function targetsOf(step: Step, facts: Facts, where: Where, short: boolean): Targets {
  const phase: Phase = stepDone(step, facts, where) && step.reveal !== "none" ? "reveal" : "spotlight";
  const area = step.area(facts, where);
  const anchor = phase === "spotlight" ? area : (step.anchor?.(facts, where) ?? []);
  const side = phase === "spotlight" ? step.side : (step.anchorSide ?? step.side);
  return {
    step,
    phase,
    hole: phase === "spotlight" ? area : step.reveal === "widen" ? [...area, ...anchor] : [],
    anchor,
    // A window too short to hold the callout above or below a card has it beside.
    side: short && (side === "top" || side === "bottom") ? "left" : side,
  };
}

/**
 * Where the targets are, kept current: read once now and again whenever the window or the callout
 * resizes, the page scrolls, or its markup changes (a pan, a pane opening, a sheet moving).
 */
function useLayout(targets: Targets | undefined, callout: RefObject<HTMLElement | null>, replied: boolean, again: number): Layout | undefined {
  const [layout, setLayout] = useState<Layout | undefined>(undefined);
  const frameState = useFrameState();
  const latest = useRef({ replied, frameState });
  latest.current = { replied, frameState };
  const [hole, anchor, step] = [targets?.hole.join("\n"), targets?.anchor.join("\n"), targets?.step];
  useEffect(() => {
    const element = callout.current;
    if (element === null || step === undefined || hole === undefined || anchor === undefined) {
      return undefined;
    }
    let [frame, opened, scrolled, reopened] = [0, step.opens === undefined, false, step.assistantPane === undefined];
    const measure = () => {
      frame = 0;
      if (!opened && step.opens !== undefined) {
        const found = step.opens.flatMap((selector) => [...document.querySelectorAll<HTMLDetailsElement>(selector)]);
        found.forEach((each) => { each.open = true; });
        opened = found.length >= step.opens.length;
      }
      // The pane is mounted only while open, so a revisit after the reply was shown finds the tab alone.
      if (!reopened && latest.current.replied && step.assistantPane !== undefined && document.querySelector(step.assistantPane) === null && latest.current.frameState?.openAssistant !== undefined) {
        reopened = true;
        latest.current.frameState.openAssistant();
      }
      const [holes, anchors] = [matches(hole === "" ? [] : hole.split("\n")), matches(anchor === "" ? [] : anchor.split("\n"))];
      if (!scrolled && (anchors[0] ?? holes[0]) !== undefined) {
        scrolled = true;
        scrollIn(anchors[0] ?? holes[0] ?? element);
      }
      const next: Layout = {
        hole: around(holes, HOLE_PAD),
        anchor: around(anchors.length === 0 ? holes : anchors, HOLE_PAD),
        viewport: { width: document.documentElement.clientWidth, height: globalThis.innerHeight },
        floor: floorOf(),
        size: { width: element.offsetWidth, height: element.offsetHeight },
      };
      setLayout((was) => (sameLayout(was, next) ? was : next));
    };
    const schedule = () => { frame ||= requestAnimationFrame(measure); };
    measure();
    const [resize, mutate] = [new ResizeObserver(schedule), new MutationObserver(schedule)];
    resize.observe(document.body);
    resize.observe(element);
    mutate.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["style", "class", "hidden", "open", "inert", "data-sheet", "aria-expanded"] });
    globalThis.addEventListener("scroll", schedule, true);
    return () => {
      cancelAnimationFrame(frame);
      resize.disconnect();
      mutate.disconnect();
      globalThis.removeEventListener("scroll", schedule, true);
    };
  }, [step, hole, anchor, callout, again]);
  return layout;
}

/** Where the callout is: beside its anchor on `side`, or docked across the foot (or, with `head`, the head) of the room. */
interface Placement {
  style: CSSProperties;
  side: Side | "dock";
  head: boolean;
}

const meets = (a: Rect, b: Rect | undefined): boolean => b !== undefined && a.left < b.left + b.width && b.left < a.left + a.width && a.top < b.top + b.height && b.top < a.top + a.height;

/** The callout's place: beside its anchor on the side asked for (or the opposite, where that has the room), else across the foot of the room above the sheet, or across its head where the foot would cover the target. */
function placeOf(layout: Layout | undefined, side: Side, docked: boolean): Placement {
  const covers = (box: Rect) => meets(box, layout?.hole) || meets(box, layout?.anchor);
  if (!docked && layout?.anchor !== undefined && layout.viewport.width >= NARROW && layout.size.width > 0) {
    const { anchor, viewport, floor, size } = layout;
    const along = side === "left" || side === "right" ? anchor.height : anchor.width;
    const at = place(anchor, size, { left: 0, top: 0, width: viewport.width, height: floor }, { side, align: along > 260 ? "start" : "center", gap: GAP, margin: MARGIN });
    if (!covers({ left: at.left, top: at.top, width: size.width, height: size.height })) {
      return { style: { left: at.left, top: at.top, "--notch-at": `${String(at.anchorOffset)}px` } as CSSProperties, side: at.side, head: false };
    }
  }
  const floor = layout?.floor ?? globalThis.innerHeight;
  const foot = { left: 0, top: floor - MARGIN - (layout?.size.height ?? 0), width: layout?.viewport.width ?? 0, height: layout?.size.height ?? 0 };
  const head = layout !== undefined && covers(foot);
  return { style: head ? { top: MARGIN, bottom: "auto" } : ({ "--dock-bottom": `${String(globalThis.innerHeight - floor + MARGIN)}px` } as CSSProperties), side: "dock", head };
}

let framings = 0;

/** Asks the canvas to keep the state's cards in view in the room the callout leaves, once per state and again when the window or the callout changes size; a step off the graph asks nothing. */
function useFraming(step: Step | undefined, facts: Facts | undefined, phase: Phase | undefined, layout: Layout | undefined, placement: Placement, again: number, setFraming: (request: Framing | undefined) => void): void {
  const cards = step === undefined || facts === undefined || step.screen !== "graph" ? [] : phase === "reveal" ? (step.revealFrame?.(facts) ?? []) : (step.frame ?? []);
  const key = cards.join("\n");
  const [width, height, floor, calloutWidth, calloutHeight, drawn] = [layout?.viewport.width, layout?.viewport.height, layout?.floor, layout?.size.width, layout?.size.height, layout?.anchor !== undefined];
  const { side, head } = placement;
  useEffect(() => {
    const canvas = document.querySelector('[data-testid="canvas"]');
    if (cards.length === 0 || canvas === null || layout === undefined || (layout.anchor === undefined && again === 0) || layout.size.width === 0) {
      setFraming(undefined);
      return;
    }
    // The canvas above the sheet, less the strip the callout takes where it is (on its side of the cards, or across the foot or head).
    const box = rectOf(canvas);
    let [left, top, right, bottom] = [box.left, box.top, box.left + box.width, Math.min(box.top + box.height, layout.floor)];
    const reserve = (side === "left" || side === "right" ? layout.size.width : layout.size.height) + GAP + HOLE_PAD + MARGIN;
    if (side === "dock" ? !head : side === "bottom") {
      bottom -= reserve;
    } else if (side === "dock" || side === "top") {
      top += reserve;
    } else if (side === "left") {
      left += reserve;
    } else {
      right -= reserve;
    }
    framings += 1;
    setFraming({ id: framings, cards, room: { left, top, width: Math.max(0, right - left), height: Math.max(0, bottom - top) } });
    // Once per state and window; the layout's other changes (a pan) are not a reason to frame again.
  }, [key, side, head, again, width, height, floor, calloutWidth, calloutHeight, drawn]);
  // Leaving the tour (or the practice journey) takes the request away.
  useEffect(() => () => { setFraming(undefined); }, [setFraming]);
}

/** The dimming around the hole, which takes the presses and the wheel, and the ring around the hole. */
function Scrim({ hole, lifted }: { hole: Rect | undefined; lifted: boolean }) {
  const [left, top, right, bottom] = hole === undefined ? [] : [hole.left, hole.top, hole.left + hole.width, hole.top + hole.height].map((each) => `${String(each)}px`);
  // One layer with the hole cut out of it, so the hole is truly open to the page.
  const dim = `polygon(evenodd, 0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${String(left)} ${String(top)}, ${String(left)} ${String(bottom)}, ${String(right)} ${String(bottom)}, ${String(right)} ${String(top)}, ${String(left)} ${String(top)})`;
  return (
    <div className="tour-scrim-layer" data-testid="tour-scrim" data-lifted={lifted} aria-hidden="true">
      {hole === undefined ? null : (
        <>
          <div className="tour-scrim" style={{ clipPath: dim }} />
          <div className="tour-ring" data-testid="tour-ring" style={{ left: hole.left, top: hole.top, width: hole.width, height: hole.height }} />
        </>
      )}
    </div>
  );
}

interface CalloutViewProps {
  refs: { callout: RefObject<HTMLElement | null>; title: RefObject<HTMLHeadingElement | null> };
  api: TourApi;
  step: Step;
  done: boolean;
  revealed: boolean;
  lifted: boolean;
  placement: Placement;
  placed: boolean;
  /** The label of the button that brings a target that is not on the screen back, when it is not. */
  showAgain: string | undefined;
  onShowAgain: () => void;
  words: string;
}

/** The callout: where the step is, what to do or what Cairn worked out, and the way on. */
function CalloutView({ refs, api, step, done, revealed, lifted, placement, placed, showAgain, onShowAgain, words }: CalloutViewProps) {
  const { steps, index } = api;
  const { side, style } = placement;
  const [titleId, bodyId] = [useId(), useId()];
  const last = index === steps.length - 1;
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
        data-side={side}
        data-placed={placed}
        {...(lifted ? {} : { "data-popover": "" })}
        style={style}
      >
        <span className="label" data-testid="tour-label">{`Tour · step ${String(index + 1)} of ${String(steps.length)} · ${step.where}`}</span>
        <h2 ref={refs.title} id={titleId} tabIndex={-1} className="tour-title" data-testid="tour-title">{step.title}</h2>
        <div className="tour-body" id={bodyId} role="status" data-testid="tour-body">
          <p>{words}</p>
        </div>
        {showAgain === undefined ? null : (
          <div className="tour-actions">
            <Button data-testid="tour-show" onClick={onShowAgain}>{showAgain}</Button>
          </div>
        )}
        <div className="tour-actions">
          {index === 0 ? null : <Button data-testid="tour-back" onClick={() => { api.arrive(index - 1); }}>Back</Button>}
          <Button primary held={done ? undefined : step.waits} data-testid="tour-next" onClick={next}>
            {last ? "Finish the tour" : "Next"}
          </Button>
          <Button ghost data-testid="tour-skip" onClick={() => { api.end("skip"); }}>Skip the tour</Button>
        </div>
      </section>
  );
}

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), summary, [tabindex]:not([tabindex="-1"])';

/** The tab stops under `root` that are drawn and not inert, in document order. */
const tabStops = (root: ParentNode): HTMLElement[] => [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((each) => each.closest("[inert]") === null && each.checkVisibility({ visibilityProperty: true }));

/** While the dimming is on, Tab goes round the callout's controls and the hole's, in that order, and never to the dimmed page; a dialog or menu opened from inside has the focus until it closes. */
function useFocusKept(callout: RefObject<HTMLElement | null>, hole: Rect | undefined, enforcing: boolean): void {
  const holeNow = useRef(hole);
  holeNow.current = hole;
  useEffect(() => {
    if (!enforcing) {
      return undefined;
    }
    const onKey = (event: KeyboardEvent) => {
      const box = holeNow.current;
      const nested = document.querySelector("dialog[open]") !== null || [...document.querySelectorAll("[data-popover]")].some((each) => each !== callout.current);
      if (event.key !== "Tab" || event.altKey || event.ctrlKey || event.metaKey || nested || callout.current === null || box === undefined) {
        return;
      }
      const inHole = (each: HTMLElement) => {
        const at = rectOf(each);
        return at.left + at.width / 2 >= box.left && at.left + at.width / 2 <= box.left + box.width && at.top + at.height / 2 >= box.top && at.top + at.height / 2 <= box.top + box.height;
      };
      const stops = [...tabStops(callout.current), ...tabStops(document.body).filter((each) => callout.current?.contains(each) !== true && inHole(each))];
      const at = stops.findIndex((each) => each === document.activeElement);
      const to = stops[at === -1 ? (event.shiftKey ? stops.length - 1 : 0) : (at + (event.shiftKey ? stops.length - 1 : 1)) % stops.length];
      if (to !== undefined) {
        event.preventDefault();
        to.focus();
      }
    };
    // Capture, so a layer that closes itself on this key is still there to be seen.
    globalThis.addEventListener("keydown", onKey, true);
    return () => { globalThis.removeEventListener("keydown", onKey, true); };
  }, [callout, enforcing]);
}

/** What arriving at a step and reaching its reveal do to the page: the reader is put at the title, and where the inspector is a sheet it is raised for a step that needs its pane, or set to the height that shows the change. */
function useArrival(step: Step, phase: Phase, title: RefObject<HTMLElement | null>): void {
  const sheet = useMedia(SHEET);
  const actions = useFrameActions();
  useEffect(() => {
    title.current?.focus();
    if (step.pane === true && sheet) {
      actions?.snap("half");
    }
  }, [step.id]);
  useEffect(() => {
    if (phase !== "reveal") {
      return;
    }
    if (step.revealSheet !== undefined && sheet) {
      actions?.snap(step.revealSheet);
    }
    // The focus stays on the page when the control that had it is gone.
    if (document.activeElement === null || document.activeElement === document.body) {
      title.current?.focus();
    }
  }, [step.id, phase]);
}

export function Overlay({ api, run }: { api: TourApi; run: string }) {
  const step = api.steps[api.index];
  return step === undefined ? null : <StepOverlay api={api} run={run} step={step} />;
}

function StepOverlay({ api, run, step }: { api: TourApi; run: string; step: Step }) {
  const { pathname, search } = useLocation();
  const [phone, short, sheet] = [useMedia(PHONE_WIDTH), useMedia(SHORT), useMedia(SHEET)];
  const actions = useFrameActions();
  const view = useJourney(run);
  const [callout, title] = [useRef<HTMLElement>(null), useRef<HTMLHeadingElement>(null)];
  const dockedFor = useRef<string | undefined>(undefined);
  const [again, setAgain] = useState(0);
  const facts = view.status === "ready" ? factsOf(view) : undefined;
  const where: Where = { selected: locate(pathname).node, frontier: viewStateFrom(new URLSearchParams(search)).frontier, replied: api.replied };
  const targets = facts === undefined ? undefined : targetsOf(step, facts, where, short);
  const layout = useLayout(targets, callout, api.replied, again);
  const phase = targets?.phase ?? "spotlight";
  useArrival(step, phase, title);
  const done = facts !== undefined && stepDone(step, facts, where);
  const lifted = phase === "reveal" && step.reveal === "lift";
  // A callout that has docked stays docked for the state and window: docked, it is wider and shorter, which could otherwise let it fit beside its target again and flip back.
  const dockKey = `${step.id}|${phase}|${String(layout?.viewport.width)}x${String(layout?.viewport.height)}`;
  const placement = placeOf(layout, targets?.side ?? step.side, dockedFor.current === dockKey);
  dockedFor.current = placement.side === "dock" ? dockKey : dockedFor.current;
  useFraming(step, facts, phase, layout, placement, again, api.setFraming);
  useFocusKept(callout, layout?.hole, !lifted && layout?.hole !== undefined);
  const words = facts === undefined ? "Reading the practice journey..." : ((phase === "reveal" ? step.result?.(facts) : undefined) ?? step.instruction(facts, { phone, short }));
  const missing = layout?.anchor === undefined;
  return (
    <>
      <Scrim hole={layout?.hole} lifted={lifted} />
      <CalloutView
        refs={{ callout, title }}
        api={api}
        step={step}
        done={done}
        revealed={phase === "reveal"}
        lifted={lifted}
        placement={placement}
        placed={!missing}
        showAgain={missing && facts !== undefined ? (step.pane === true && sheet ? "Raise the inspector" : "Show the control") : undefined}
        onShowAgain={() => { if (step.pane === true && sheet) { actions?.snap("half"); } else { setAgain((count) => count + 1); } }}
        words={words}
      />
    </>
  );
}
