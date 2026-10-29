// A button that opens a small menu of actions: the View picker and the journey's `⋯` in its
// header, and the toolbar's filter. It closes on Escape, on a click outside it, and when an item
// is chosen. Its popover is marked `data-popover`, which the keyboard map reads: an Escape that
// finds one open leaves the selection alone (3.4).
//
// A menu of actions (`role="menu"`) is a keyboard menu: opened from the keyboard it puts the
// focus on its current item (`aria-current="page"`) or its first; the arrow keys move over the
// enabled items, wrapping round, Home and End jump; Esc, choosing an item and Tab close it and
// hand the focus back to its button (Tab then moves on from the button). A popover holding
// controls (`role="dialog"`, the filter) is left to the controls. A popover that would run off
// either edge of the window is slid back inside it, one that does not fit below its button opens
// upward when there is more room there, and one taller than its room scrolls, so no item is out
// of reach on a phone or in the inspector and the page never grows to hold it.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode, type RefObject } from "react";

import { menuFocusTarget } from "../ui/menuKeys.ts";
import { useOpenedOnRequest } from "../ui/reveal.ts";
import { place, windowBounds } from "../ui/place.ts";
import { Tip } from "../ui/Tip.tsx";

/** The space a popover keeps from the window's edge (px). */
const EDGE_PX = 16;

/** The vertical span a popover can show in: the nearest ancestor that clips or scrolls it, or the window. */
function roomOf(popover: HTMLElement): { top: number; bottom: number } {
  for (let each = popover.parentElement; each !== null; each = each.parentElement) {
    if (getComputedStyle(each).overflowY !== "visible") {
      const box = each.getBoundingClientRect();
      return { top: Math.max(box.top, 0), bottom: Math.min(box.bottom, window.innerHeight) };
    }
  }
  return { top: 0, bottom: window.innerHeight };
}

/**
 * Keeps the popover in view when it opens, by the app's one placement (`place()`): slid back from
 * a side of the window it runs past, as tall as the room below its button, or above it when that
 * is more and the popover does not fit below (the inspector's action row sits low in a column that
 * clips it). The popover stays under its button in the DOM, so this only moves it (a `translateX`
 * for the slide, `data-side` for the flip).
 */
function useInWindow(popover: RefObject<HTMLDivElement | null>, body: RefObject<HTMLDivElement | null>, align: "start" | "end", open: boolean): void {
  useLayoutEffect(() => {
    const box = popover.current?.getBoundingClientRect();
    const button = popover.current?.parentElement?.getBoundingClientRect();
    if (popover.current === null || box === undefined || button === undefined) {
      return;
    }
    const room = roomOf(popover.current);
    const at = place(button, box, { ...windowBounds(), top: room.top, height: room.bottom - room.top }, { side: "bottom", align, gap: 0, margin: EDGE_PX });
    const slide = at.left - box.left;
    if (slide !== 0) {
      popover.current.style.setProperty("transform", `translateX(${String(slide)}px)`);
    }
    popover.current.dataset["side"] = at.side;
    // DESIGN, Popover and menu: the body scrolls within the room; the stylesheet's cap (--popover-body-max) holds it to that room or less.
    body.current?.style.setProperty("--popover-room", `${String(Math.max(at.room, 0))}px`);
    if (body.current !== null) {
      cutBeforeHeading(body.current);
    }
  }, [popover, body, align, open]);
}

/** The room a heading needs under it to be worth showing: one option, a touch target tall. */
const HEADING_ROOM_PX = 44;

/** A body that scrolls is cut above a heading that would be its last visible row without an option under it, so a section is never shown by its name alone. */
function cutBeforeHeading(body: HTMLElement): void {
  if (body.scrollHeight <= body.clientHeight) {
    return;
  }
  const box = body.getBoundingClientRect();
  for (const heading of body.querySelectorAll<HTMLElement>(".label, legend")) {
    const at = heading.getBoundingClientRect();
    if (at.top > box.top + HEADING_ROOM_PX && at.top < box.bottom && at.bottom + HEADING_ROOM_PX > box.bottom) {
      body.style.setProperty("--popover-room", `${String(Math.floor(at.top - box.top))}px`);
      return;
    }
  }
}

/** The items of a menu of actions that can take the focus (a disabled button cannot), in the order drawn. */
function enabledItems(popover: HTMLElement | null): HTMLElement[] {
  // A held item (aria-disabled) is reached, so its reason can be read.
  return [...(popover?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? [])];
}

/** Closes an open menu on a press outside `root`, and on Escape (`escape`, which hands the focus back). */
function useDismiss(open: boolean, root: RefObject<HTMLElement | null>, close: () => void, escape: () => void): void {
  useEffect(() => {
    if (!open) {
      return;
    }
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && root.current?.contains(event.target) !== true) {
        close();
      }
    };
    const onEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        escape();
      }
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", onEscape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", onEscape);
    };
  }, [open, root, close, escape]);
}

/**
 * The keyboard half of a menu of actions (`role="menu"`): the button opens it on ArrowDown, a
 * keyboard open puts the focus on the current or first enabled item once the popover is placed,
 * the arrows, Home and End move over the enabled items, and Tab closes it with the focus handed
 * to the button. Nothing for a popover of controls.
 */
function useMenuKeys(
  keyboardMenu: boolean,
  open: boolean,
  setOpen: (open: boolean) => void,
  button: RefObject<HTMLButtonElement | null>,
  popover: RefObject<HTMLDivElement | null>,
) {
  // Set when the keyboard opened the menu: the focus then goes into it.
  const fromKeyboard = useRef(false);
  useLayoutEffect(() => {
    if (open && fromKeyboard.current && keyboardMenu) {
      const items = enabledItems(popover.current);
      (items.find((item) => item.getAttribute("aria-current") === "page") ?? items[0])?.focus();
    }
    fromKeyboard.current = false;
  }, [open, keyboardMenu, popover]);
  const onButtonKeyDown = (event: ReactKeyboardEvent) => {
    if (keyboardMenu && event.key === "ArrowDown") {
      event.preventDefault();
      if (open) {
        enabledItems(popover.current)[0]?.focus();
      } else {
        fromKeyboard.current = true;
        setOpen(true);
      }
    }
  };
  const onPopoverKeyDown = (event: ReactKeyboardEvent) => {
    if (!keyboardMenu) {
      return;
    }
    if (event.key === "Tab") {
      // The default stays: the browser moves on from the button, not from an item that is gone.
      button.current?.focus();
      setOpen(false);
      return;
    }
    const items = enabledItems(popover.current);
    const to = menuFocusTarget(event.key, items.indexOf(document.activeElement as HTMLElement), items.length);
    if (to !== undefined) {
      event.preventDefault();
      items[to]?.focus();
    }
  };
  /** A click with no pointer (Enter, Space) is the keyboard's. */
  const onClick = (event: { detail: number }) => {
    fromKeyboard.current = !open && event.detail === 0;
    setOpen(!open);
  };
  return { onButtonKeyDown, onPopoverKeyDown, onClick };
}

interface MenuProps {
  /** What the button shows. */
  trigger: ReactNode;
  /** The button's accessible name. */
  label: string;
  /** The button's tooltip: what it does. Left out when its label already says it. */
  tip?: ReactNode;
  /** `name` for an icon-only button, whose tooltip shows its accessible name. */
  tipKind?: "describe" | "name";
  /** The key that opens it, drawn in the tooltip. */
  keys?: string;
  testId: string;
  align?: "start" | "end";
  className?: string;
  /** `menu` for a list of actions; `dialog` for a popover holding controls (the filter). */
  role?: "menu" | "dialog";
  tour?: string;
  children: (close: () => void) => ReactNode;
}

/** The menu's items are drawn by `children`, given the way to close it. */
export function Menu({ trigger, label, tip, tipKind = "describe", keys, testId, align = "start", className = "", role = "menu", tour, children }: MenuProps) {
  const [open, setOpen] = useState(false);
  useOpenedOnRequest(testId, useCallback(() => { setOpen(true); }, []));
  const root = useRef<HTMLSpanElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const body = useRef<HTMLDivElement>(null);
  // Placed first: focus moves into the popover only once it is placed.
  useInWindow(popover, body, align, open);
  const keyboard = useMenuKeys(role === "menu", open, setOpen, button, popover);
  const close = useCallback(() => { setOpen(false); }, []);
  // A close the person made inside the popover hands the focus back to the button, which is
  // where it would otherwise be lost when the popover unmounts under it.
  const closeToButton = useCallback(() => {
    if (popover.current?.contains(document.activeElement) === true) {
      button.current?.focus();
    }
    setOpen(false);
  }, []);
  useDismiss(open, root, close, closeToButton);
  return (
    <span ref={root} className={`dropdown dropdown-${align} ${className}`.trim()}>
      <Tip tip={tip} kind={tipKind} {...(keys === undefined ? {} : { keys })}>
        <button
          ref={button}
          type="button"
          className="button dropdown-button"
          aria-haspopup={role}
          aria-expanded={open}
          aria-label={label}
          data-testid={testId}
          {...(tour === undefined ? {} : { "data-tour": tour })}
          onKeyDown={keyboard.onButtonKeyDown}
          onClick={keyboard.onClick}
        >
          {trigger}
        </button>
      </Tip>
      {open ? (
        <div ref={popover} className="popover dropdown-popover" role={role} aria-label={label} data-popover="" data-overhang="" data-testid={`${testId}-menu`} onKeyDown={keyboard.onPopoverKeyDown}>
          <div ref={body} className="popover-body stack">
            {children(closeToButton)}
          </div>
        </div>
      ) : null}
    </span>
  );
}
