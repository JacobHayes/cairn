// PRD: C20.
// Keyboard focus under a spotlight. The callout is the app's open layer (it carries `data-popover`,
// so every handler that asks `overlayOpen()` stands down), and while it asks for an action the
// focus is kept, not merely placed: Tab moves between the callout's controls and the action area's
// and wraps from one to the other, and a focus that lands anywhere else returns to the callout's
// title. It is not a modal (a screen reader's cursor may still read the dimmed page), and it steps
// aside for a native dialog or a menu opened from inside the area. Escape moves the focus to the
// title and does nothing else; it is never Skip.
import { useEffect, type RefObject } from "react";

import type { Rect } from "../ui/place.ts";
import { holds } from "./track.ts";

/**
 * Where Tab (or Shift+Tab) from `active` goes, when the browser's own next stop would leave the
 * callout and the area: the order is the callout's controls, then the area's, then the callout's
 * again. None means the browser moves as it would.
 */
export function tabDestination<T>({ callout, area, active, shift }: { callout: readonly T[]; area: readonly T[]; active: T | undefined; shift: boolean }): T | undefined {
  const [calloutFirst, calloutLast, areaFirst, areaLast] = [callout[0], callout.at(-1), area[0], area.at(-1)];
  if (active === undefined || (!callout.includes(active) && !area.includes(active))) {
    // From anywhere else, back into the callout.
    return shift ? (calloutLast ?? areaLast) : (calloutFirst ?? areaFirst);
  }
  if (shift) {
    if (active === calloutFirst) {
      return areaLast ?? calloutLast;
    }
    return active === areaFirst ? calloutLast : undefined;
  }
  if (active === calloutLast) {
    return areaFirst ?? calloutFirst;
  }
  return active === areaLast ? calloutFirst : undefined;
}

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), summary, [tabindex]:not([tabindex="-1"]), [contenteditable="true"]';

/** The tab stops under `root` that are drawn, not inert, in document order. */
function tabStops(root: ParentNode): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((each) => each.tabIndex >= 0 && each.closest("[inert]") === null && each.checkVisibility({ visibilityProperty: true }));
}

/** Whether the element's centre is in the hole: what the action area holds, as a person sees it. */
function inside(element: Element, hole: Rect): boolean {
  const box = element.getBoundingClientRect();
  return holds(hole, box.left + box.width / 2, box.top + box.height / 2);
}

export interface FocusKeeping {
  callout: RefObject<HTMLElement | null>;
  title: RefObject<HTMLElement | null>;
  /** The hole, in the window's coordinates; none while the area is not on the screen. */
  hole: () => Rect | undefined;
  /** The focus is kept (a spotlight, or a widened reveal), and not a lifted one. */
  enforcing: boolean;
}

/** Keeps the focus between the callout and the area for as long as `enforcing`. */
export function useFocusKeeper({ callout, title, hole, enforcing }: FocusKeeping): void {
  useEffect(() => {
    if (!enforcing) {
      return undefined;
    }
    const controls = () => (callout.current === null ? [] : tabStops(callout.current));
    const area = () => {
      const box = hole();
      return box === undefined ? [] : tabStops(document.body).filter((each) => callout.current?.contains(each) !== true && inside(each, box));
    };
    // A native dialog, or a menu or popover another element opened, has the focus until it closes.
    const stepsAside = () => document.querySelector("dialog[open]") !== null || [...document.querySelectorAll("[data-popover]")].some((each) => each !== callout.current);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        // The nested layer closes itself first; with the callout alone left, the focus goes to its title and nothing else does.
        if (!stepsAside()) {
          title.current?.focus();
        }
        return;
      }
      if (event.key !== "Tab" || event.altKey || event.ctrlKey || event.metaKey || stepsAside()) {
        return;
      }
      const active = document.activeElement instanceof HTMLElement ? document.activeElement : undefined;
      const to = tabDestination({ callout: controls(), area: area(), active, shift: event.shiftKey });
      if (to !== undefined) {
        event.preventDefault();
        to.focus();
      }
    };
    const onFocus = (event: FocusEvent) => {
      const target = event.target;
      const box = hole();
      if (!(target instanceof Element) || callout.current?.contains(target) === true || stepsAside() || box === undefined || inside(target, box) || target.closest("[data-popover]") !== null) {
        return;
      }
      title.current?.focus();
    };
    // Capture, so a layer that closes itself on this key is still there to be seen.
    globalThis.addEventListener("keydown", onKey, true);
    document.addEventListener("focusin", onFocus);
    return () => {
      globalThis.removeEventListener("keydown", onKey, true);
      document.removeEventListener("focusin", onFocus);
    };
  }, [callout, title, hole, enforcing]);
}
