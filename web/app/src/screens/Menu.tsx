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

/** The space a popover keeps from the window's edge (px). */
const EDGE_PX = 16;
/** The tallest a popover's body grows before it scrolls (DESIGN, Popover and menu). */
const BODY_PX_MAX = 480;

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
 * Keeps the popover in view when it opens: slid back from a side of the window it runs past,
 * and as tall as the room below its button, or above it when that is more and the popover
 * does not fit below (the inspector's action row sits low in a column that clips it).
 */
function useInWindow(popover: RefObject<HTMLDivElement | null>, body: RefObject<HTMLDivElement | null>, open: boolean): void {
  useLayoutEffect(() => {
    const box = popover.current?.getBoundingClientRect();
    const button = popover.current?.parentElement?.getBoundingClientRect();
    if (popover.current === null || box === undefined || button === undefined) {
      return;
    }
    const width = document.documentElement.clientWidth;
    const slide = Math.max(EDGE_PX - box.left, Math.min(0, width - EDGE_PX - box.right));
    if (slide !== 0) {
      popover.current.style.setProperty("transform", `translateX(${String(slide)}px)`);
    }
    const room = roomOf(popover.current);
    const below = room.bottom - box.top - EDGE_PX;
    const above = button.top - room.top - EDGE_PX;
    const flip = box.height > below && above > below;
    popover.current.dataset["side"] = flip ? "top" : "bottom";
    // DESIGN, Popover and menu: the body scrolls within the room, and never past min(60dvh, 480px).
    const cap = Math.min(window.innerHeight * 0.6, BODY_PX_MAX);
    body.current?.style.setProperty("max-height", `${String(Math.max(Math.min(flip ? above : below, cap), 0))}px`);
  }, [popover, body, open]);
}

/** The items of a menu of actions that can take the focus (a disabled button cannot), in the order drawn. */
function enabledItems(popover: HTMLElement | null): HTMLElement[] {
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

/** The menu's items are drawn by `children`, given the way to close it. */
export function Menu({
  trigger,
  label,
  title,
  testId,
  align = "start",
  className = "",
  role = "menu",
  children,
}: {
  /** What the button shows. */
  trigger: ReactNode;
  /** The button's accessible name. */
  label: string;
  /** A tooltip for the button. */
  title?: string;
  testId: string;
  align?: "start" | "end";
  className?: string;
  /** `menu` for a list of actions; `dialog` for a popover holding controls (the filter). */
  role?: "menu" | "dialog";
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLSpanElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const body = useRef<HTMLDivElement>(null);
  // Placed first: the focus moves into the popover only once it is where it will be.
  useInWindow(popover, body, open);
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
      <button
        ref={button}
        type="button"
        className="button dropdown-button"
        aria-haspopup={role}
        aria-expanded={open}
        aria-label={label}
        {...(title === undefined ? {} : { title })}
        data-testid={testId}
        onKeyDown={keyboard.onButtonKeyDown}
        onClick={keyboard.onClick}
      >
        {trigger}
      </button>
      {open ? (
        <div ref={popover} className="popover dropdown-popover" role={role} data-popover="" data-testid={`${testId}-menu`} onKeyDown={keyboard.onPopoverKeyDown}>
          <div ref={body} className="popover-body stack">
            {children(closeToButton)}
          </div>
        </div>
      ) : null}
    </span>
  );
}
