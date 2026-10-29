// The one tooltip (DESIGN, Tooltip). `Tip` wraps one element and gives it a tip; `TipLayer`,
// mounted once at the shell root, shows it. A tip is a `popover="manual"` element next to its
// trigger, so it sits in the browser's top layer (above modals, outside every scroller and canvas
// transform) with no portal and no z-index; it holds no React state, so showing one never
// re-renders its trigger. The layer listens on the document for pointer, focus, keys and scrolls,
// runs the timing machine (tip.ts) and places the box with `place()` (place.ts). A thing with no
// box to hang a tip on (a graph line, the timeline's end line) uses `usePointTip()` instead, and
// the layer draws its content in an element of its own.
import { useActorRef, useSelector } from "@xstate/react";
import { cloneElement, createContext, isValidElement, use, useEffect, useId, useMemo, useRef, type ReactElement, type ReactNode } from "react";
import type { ActorRefFrom } from "xstate";

import { place, pointerRect, windowBounds, type PlaceOptions, type Rect } from "./place.ts";
import { tipMachine, tipShown, tipTarget, type TipTarget, type TipVia } from "./tip.ts";

/**
 * What the tip says about its trigger: `describe` is its description (what it does, what a badge counts, why it is held); `name` shows the accessible name of an icon-only control and is hidden from assistive technology (the name already reaches it); `cut` shows the whole of a text only while it is cut short, and is hidden from assistive technology too (the whole text is in the page).
 */
export type TipKind = "describe" | "name" | "cut";

export interface TipProps {
  /** The words: a few lines at most, never a control. Nothing (undefined, null, false or "") means no tip. */
  tip: ReactNode;
  /** The key this triggers, drawn as a `kbd` chip at the right and set as the trigger's `aria-keyshortcuts`. */
  keys?: string;
  kind?: TipKind;
  /** A fact whose only place is this tip, or a held control's reason: the trigger is reachable by keyboard, and a tap, a click, Enter or Space shows the tip. */
  tap?: boolean;
  /** `panel` is the keys hint's preview of the key sheet, drawn as a popover. */
  look?: "chip" | "panel";
  /** The one element (or component that forwards its props to its root) the tip describes. */
  children: ReactElement<Record<string, unknown>>;
}

/** `Shift F` in the words people read is `Shift+F` in `aria-keyshortcuts`. */
const shortcutOf = (keys: string): string => keys.replaceAll(" ", "+");

const CHIP: PlaceOptions = { side: "bottom", align: "center", gap: 6, margin: 8 };
const PANEL: PlaceOptions = { side: "right", align: "end", gap: 0, margin: 12 };
/** A fact tapped on a phone: above it, where the finger is not and the line below it stays readable. */
const TAPPED: PlaceOptions = { side: "top", align: "center", gap: 6, margin: 8 };
const POINT: PlaceOptions = { side: "bottom", align: "start", gap: 4, margin: 8 };

const hasWords = (tip: ReactNode): boolean => tip !== undefined && tip !== null && tip !== false && tip !== "";

export function Tip({ tip, keys, kind = "describe", tap = false, look = "chip", children }: TipProps) {
  const id = useId();
  if (!hasWords(tip) || !isValidElement(children)) {
    return children;
  }
  const own = children.props;
  const described = kind === "describe" && look === "chip";
  const props: Record<string, unknown> = {
    "data-tip": id,
    ...(kind === "describe" ? {} : { "data-tip-kind": kind }),
    // A cut text is not a tab stop: a keyboard reaches the whole text where it lives.
    ...(tap ? { "data-tip-tap": "", ...(own["tabIndex"] === undefined && kind !== "cut" ? { tabIndex: 0 } : {}) } : {}),
    ...(look === "panel" ? { "data-tip-look": "panel" } : {}),
    ...(described ? { "aria-describedby": [own["aria-describedby"], id].filter((each) => typeof each === "string" && each !== "").join(" ") } : {}),
    ...(keys === undefined ? {} : { "aria-keyshortcuts": shortcutOf(keys) }),
  };
  return (
    <>
      {cloneElement(children, props)}
      <span id={id} popover="manual" role="tooltip" className={look === "panel" ? "tip popover" : "tip"} data-overhang="" data-look={look} data-testid="tooltip" {...(described ? {} : { "aria-hidden": true })}>
        {look === "panel" ? tip : <span className="tip-words">{tip}</span>}
        {keys === undefined ? null : <kbd aria-hidden="true">{keys}</kbd>}
      </span>
    </>
  );
}

// ---- point tips ---------------------------------------------------------------------------

/** What a thing without a box tells the layer. Each takes the thing's `id` (a different id is a different tip). */
export interface PointTip {
  /** The pointer rests here (every move over the thing calls it again; the tip opens where the rest settled). */
  hoverAt: (id: string, x: number, y: number, content: ReactNode) => void;
  /** Keyboard focus is on the thing, whose box is `rect`. */
  focusOn: (id: string, rect: Rect, content: ReactNode) => void;
  /** A tap: shows the tip at the point until the next tap. */
  tapAt: (id: string, x: number, y: number, content: ReactNode) => void;
  /** The pointer left. */
  leave: () => void;
  /** The focus left. */
  blur: () => void;
}

type TipActor = ActorRefFrom<typeof tipMachine>;

const TipContext = createContext<TipActor | undefined>(undefined);

/** For a thing that has no box to hang a sibling on (a graph line): the same events a trigger sends, with a point instead of an element. */
export function usePointTip(): PointTip {
  const actor = use(TipContext);
  const used = useRef(new Set<string>());
  // What this view's lines opened goes when the view does: a point target has no element for the layer to see disconnected.
  useEffect(() => {
    const ids = used.current;
    return () => {
      for (const id of ids) {
        actor?.send({ type: "drop", id });
      }
    };
  }, [actor]);
  return useMemo<PointTip>(() => {
    const send = actor?.send.bind(actor);
    return {
      hoverAt: (id, x, y, content) => { used.current.add(id); send?.({ type: "hover", target: { id, box: pointerRect(x, y), content } }); },
      focusOn: (id, rect, content) => { used.current.add(id); send?.({ type: "focus", target: { id, box: rect, content } }); },
      tapAt: (id, x, y, content) => { used.current.add(id); send?.({ type: "tap", target: { id, box: pointerRect(x, y), content } }); },
      leave: () => { send?.({ type: "unhover" }); },
      blur: () => { send?.({ type: "blur" }); },
    };
  }, [actor]);
}

// ---- the layer ----------------------------------------------------------------------------

const TRIGGER = "[data-tip]";
const NATIVE_KEYED = "button, a[href], input, select, textarea, summary";
const FIELDS = "input:not([type=checkbox], [type=radio], [type=button], [type=submit]), textarea, select, [contenteditable]";

/** The trigger an event's target belongs to, or the trigger of the tip it is on. */
function triggerOf(from: EventTarget | null): HTMLElement | undefined {
  if (!(from instanceof Element)) {
    return undefined;
  }
  const tip = from.closest<HTMLElement>(".tip");
  if (tip !== null) {
    return tip.id === "" ? undefined : (document.querySelector<HTMLElement>(`[data-tip="${CSS.escape(tip.id)}"]`) ?? undefined);
  }
  return from.closest<HTMLElement>(TRIGGER) ?? undefined;
}

/** Whether a text is cut short: it or something in it overflows its box. */
function isCut(trigger: HTMLElement): boolean {
  const cut = (each: HTMLElement) => each.scrollWidth > each.clientWidth + 1 || each.scrollHeight > each.clientHeight + 1;
  return cut(trigger) || [...trigger.querySelectorAll<HTMLElement>("*")].some(cut);
}

const targetOfElement = (trigger: HTMLElement): TipTarget => ({ id: trigger.dataset["tip"] ?? "", element: trigger });

/** Whether a trigger may show its tip now: a cut-text tip only while the text is cut. */
const ready = (trigger: HTMLElement): boolean => trigger.dataset["tipKind"] !== "cut" || isCut(trigger);

/** The one layer: mount it once, at the shell root. */
export function TipLayer({ children }: { children?: ReactNode }) {
  const actor = useActorRef(tipMachine);
  const point = useSelector(actor, (snapshot) => {
    const shown = tipTarget(snapshot);
    return shown?.target.element === undefined && shown?.target.content !== undefined ? shown.target : undefined;
  });
  const presenter = useMemo(() => presenterOf(actor), [actor]);
  useEffect(() => {
    const unsubscribe = actor.subscribe(() => { presenter.sync(false); });
    const stop = layerListeners(actor, presenter);
    return () => {
      unsubscribe.unsubscribe();
      stop();
    };
  }, [actor, presenter]);
  // A point tip's words are drawn by this render: it opens and is placed once they are in.
  useEffect(() => { presenter.sync(true); }, [point, presenter]);
  return (
    <TipContext value={actor}>
      {children}
      <span id={POINT_ID} popover="manual" role="tooltip" className="tip" data-overhang="" data-testid="tooltip" aria-hidden="true">
        <span className="tip-words">{point?.content}</span>
      </span>
    </TipContext>
  );
}

const POINT_ID = "tip-point";

/** The tip element of the shown target: its own, or the layer's for a point. */
function tipElementOf(target: TipTarget): HTMLElement | null {
  return document.getElementById(target.element === undefined ? POINT_ID : target.id);
}

interface Presenter {
  /** Opens what the machine says is shown and closes what it does not; `drawn` is true once a point tip's words are in the page. */
  sync: (drawn: boolean) => void;
  /** Places the open tip against its trigger again (a scroll or resize). */
  replace: () => void;
}

/** Puts the machine's tip on the page: showPopover, place(), hidePopover. */
function presenterOf(actor: TipActor): Presenter {
  let open: { id: string; element: HTMLElement } | undefined;
  const close = () => {
    if (open?.element.isConnected === true && open.element.matches(":popover-open")) {
      open.element.hidePopover();
    }
    open = undefined;
  };
  const placeOpen = (target: TipTarget, element: HTMLElement, via: TipVia) => {
    const anchor = target.element === undefined ? target.box : target.element.getBoundingClientRect();
    if (anchor === undefined) {
      return;
    }
    // Measured at the corner, so its width is what the room allows and not what is left of where it was.
    element.style.setProperty("left", "0px");
    element.style.setProperty("top", "0px");
    const box = element.getBoundingClientRect();
    const options = element.dataset["look"] === "panel" ? PANEL : target.element === undefined ? POINT : via === "tap" ? TAPPED : CHIP;
    const at = place(anchor, { width: box.width, height: box.height }, windowBounds(), options);
    element.style.setProperty("left", `${String(at.left)}px`);
    element.style.setProperty("top", `${String(at.top)}px`);
    element.dataset["side"] = at.side;
  };
  return {
    sync: (drawn) => {
      const shown = tipTarget(actor.getSnapshot());
      const element = shown === undefined ? null : tipElementOf(shown.target);
      if (open !== undefined && (shown === undefined || element !== open.element || shown.target.id !== open.id)) {
        close();
      }
      if (shown === undefined || element === null || open !== undefined || !(shown.target.element === undefined ? drawn : shown.target.element.isConnected)) {
        return;
      }
      element.showPopover();
      open = { id: shown.target.id, element };
      placeOpen(shown.target, element, shown.via);
    },
    replace: () => {
      const shown = tipTarget(actor.getSnapshot());
      if (open !== undefined && shown?.target.element?.isConnected === true) {
        placeOpen(shown.target, open.element, shown.via);
      }
    },
  };
}

/** What every group of listeners shares: the machine, the presenter, the listeners' signal, and which trigger the pointer is on. */
interface Listening {
  actor: TipActor;
  presenter: Presenter;
  signal: AbortSignal;
  hovered: { id: string | undefined };
}

const isShown = (actor: TipActor): boolean => tipShown(actor.getSnapshot());

/** A trigger that has left the page closes its tip. */
function sweep({ actor }: Listening): void {
  const target = actor.getSnapshot().context.target;
  if (target?.element !== undefined && !target.element.isConnected) {
    actor.send({ type: "dismiss" });
  }
}

function unhover({ actor, hovered }: Listening): void {
  if (hovered.id !== undefined) {
    hovered.id = undefined;
    actor.send({ type: "unhover" });
  }
}

/** The pointer: a rest on a trigger (or on the point tip, which keeps it) shows its tip, leaving takes it away. */
function pointerListeners(listening: Listening): void {
  const { actor, signal, hovered } = listening;
  document.addEventListener(
    "pointermove",
    (event) => {
      sweep(listening);
      if (event.pointerType === "touch" || event.buttons !== 0) {
        return;
      }
      // The point tip has no trigger to find: the pointer on it keeps it.
      if (event.target instanceof Element && event.target.closest(`#${POINT_ID}`) !== null) {
        const target = actor.getSnapshot().context.target;
        if (target !== undefined && isShown(actor)) {
          hovered.id = target.id;
          actor.send({ type: "hover", target });
        }
        return;
      }
      const trigger = triggerOf(event.target);
      if (trigger === undefined || !ready(trigger)) {
        unhover(listening);
        return;
      }
      hovered.id = trigger.dataset["tip"];
      actor.send({ type: "hover", target: targetOfElement(trigger) });
    },
    { signal },
  );
  document.addEventListener(
    "pointerout",
    (event) => {
      if (event.relatedTarget === null && event.pointerType !== "touch") {
        unhover(listening);
      }
    },
    { signal },
  );
}

/** Keyboard focus: a focus-visible trigger shows its tip at once (a text field's is hover-only), and leaving it takes it away. */
function focusListeners({ actor, signal }: Listening): void {
  document.addEventListener(
    "focusin",
    (event) => {
      const trigger = triggerOf(event.target);
      if (trigger !== undefined && event.target instanceof HTMLElement && event.target.matches(":focus-visible") && !trigger.matches(FIELDS) && ready(trigger)) {
        actor.send({ type: "focus", target: targetOfElement(trigger) });
      }
    },
    { signal },
  );
  document.addEventListener(
    "focusout",
    () => {
      actor.send({ type: "blur" });
    },
    { signal },
  );
}

/** What a toggletip's trigger is sent when it is tapped by any means (a pointer, a key, a screen reader's click): its own click never reaches it, since the tip takes the click. */
export const TIP_TAPPED = "cairn:tip-tapped";

/** A press elsewhere dismisses; a click on a toggletip shows it and does nothing else. */
function pressListeners({ actor, signal }: Listening): void {
  document.addEventListener(
    "pointerdown",
    (event) => {
      const trigger = triggerOf(event.target);
      const onTip = event.target instanceof Element && event.target.closest(".tip") !== null;
      // A press on a toggletip is the start of its tap.
      if (!onTip && trigger?.dataset["tipTap"] === undefined) {
        actor.send({ type: "dismiss" });
      }
    },
    { capture: true, signal },
  );
  document.addEventListener(
    "click",
    (event) => {
      const trigger = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-tip-tap]") : null;
      if (trigger !== null && ready(trigger)) {
        // A tap on a fact shows it and does nothing else: it does not follow the link or open the row around it.
        event.preventDefault();
        event.stopPropagation();
        actor.send({ type: "tap", target: targetOfElement(trigger) });
        trigger.dispatchEvent(new Event(TIP_TAPPED));
      }
    },
    { capture: true, signal },
  );
}

/** Esc dismisses and nothing more; Enter or Space on a toggletip shows it, and on any other trigger gets the tip out of the way. */
function keyListeners(listening: Listening): void {
  const { actor, signal } = listening;
  document.addEventListener(
    "keydown",
    (event) => {
      sweep(listening);
      if (event.key === "Escape" && (isShown(actor) || actor.getSnapshot().matches("waiting"))) {
        // A second Esc does its usual work.
        const was = isShown(actor);
        actor.send({ type: "dismiss" });
        if (was) {
          event.stopPropagation();
          event.preventDefault();
        }
        return;
      }
      const trigger = event.key === "Enter" || event.key === " " ? triggerOf(event.target) : undefined;
      if (trigger === undefined || event.target !== trigger) {
        return;
      }
      if (trigger.dataset["tipTap"] === undefined) {
        // Being pressed: the tip gets out of the way.
        actor.send({ type: "dismiss" });
      } else if (!trigger.matches(NATIVE_KEYED)) {
        event.preventDefault();
        event.stopPropagation();
        if (!event.repeat) {
          actor.send({ type: "tap", target: targetOfElement(trigger) });
        }
      }
    },
    { capture: true, signal },
  );
}

/** Something moved under a still pointer: its hover tip goes; a focus or tap tip follows its trigger. */
function moveListeners({ actor, presenter, signal }: Listening): void {
  const away = (event: Event) => {
    if (event.target instanceof Element && event.target.closest(".tip") !== null) {
      return;
    }
    // A scroll moves what is under the pointer only when the scroller holds the thing: another scroller's (a column resetting to its top, a list loading in) leaves it where it was.
    const held = actor.getSnapshot().context.target?.element;
    if (event.type === "scroll" && held !== undefined && event.target instanceof Node && !event.target.contains(held)) {
      return;
    }
    if (actor.getSnapshot().context.via === "pointer") {
      actor.send({ type: "dismiss" });
    } else {
      presenter.replace();
    }
  };
  document.addEventListener("scroll", away, { capture: true, passive: true, signal });
  document.addEventListener("wheel", away, { passive: true, signal });
  window.addEventListener("resize", away, { signal });
}

/** The document's listeners for one layer. */
function layerListeners(actor: TipActor, presenter: Presenter): () => void {
  const abort = new AbortController();
  const listening: Listening = { actor, presenter, signal: abort.signal, hovered: { id: undefined } };
  for (const listen of [pointerListeners, focusListeners, pressListeners, keyListeners, moveListeners]) {
    listen(listening);
  }
  return () => {
    abort.abort();
    actor.send({ type: "dismiss" });
    presenter.sync(false);
  };
}
