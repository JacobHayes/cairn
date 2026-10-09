// A button that opens a small menu of actions: the lifecycle chip and the journey's `⋯` in
// its header, and the toolbar's filter. It closes on Escape, on a click outside it, and when
// an item is chosen. Its popover is marked `data-popover`, which the keyboard map reads: an
// Escape that finds one open leaves the selection alone (3.4). A popover that would run off
// either edge of the window is slid back inside it, one that does not fit below its button opens
// upward when there is more room there, and one taller than its room scrolls, so no item is out
// of reach on a phone or in the inspector and the page never grows to hold it.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";

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
    body.current?.style.setProperty("max-height", `${String(Math.max(flip ? above : below, 0))}px`);
  }, [popover, body, open]);
}

/** Closes an open menu on Escape and on a press outside `root`. */
function useDismiss(open: boolean, root: RefObject<HTMLElement | null>, close: () => void): void {
  useEffect(() => {
    if (!open) {
      return;
    }
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && root.current?.contains(event.target) !== true) {
        close();
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        close();
      }
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [open, root, close]);
}

/** The menu's items are drawn by `children`, given the way to close it. */
export function Menu({
  trigger,
  label,
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
  testId: string;
  align?: "start" | "end";
  className?: string;
  /** `menu` for a list of actions; `dialog` for a popover holding controls (the filter). */
  role?: "menu" | "dialog";
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLSpanElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const body = useRef<HTMLDivElement>(null);
  useInWindow(popover, body, open);
  const close = useCallback(() => { setOpen(false); }, []);
  useDismiss(open, root, close);
  return (
    <span ref={root} className={`dropdown dropdown-${align} ${className}`.trim()}>
      <button
        type="button"
        className="button dropdown-button"
        aria-haspopup={role}
        aria-expanded={open}
        aria-label={label}
        data-testid={testId}
        onClick={() => {
          setOpen(!open);
        }}
      >
        {trigger}
      </button>
      {open ? (
        <div ref={popover} className="popover dropdown-popover" role={role} data-popover="" data-testid={`${testId}-menu`}>
          <div ref={body} className="popover-body stack">
            {children(() => {
              setOpen(false);
            })}
          </div>
        </div>
      ) : null}
    </span>
  );
}
