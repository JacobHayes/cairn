// Pointing at a form that is not on screen: a popover closed under the press that left it, or a
// node's panel for another node. The menu with that test id is asked to open, and the form's first
// field takes the focus as soon as it is drawn, with any fold around it opened.
import { useEffect } from "react";

const MENU_OPEN = "cairn:open-menu";

/** Asks the menu whose button has `testId` to open. */
export function openMenu(testId: string): void {
  window.dispatchEvent(new CustomEvent(MENU_OPEN, { detail: testId }));
}

/** Calls `open` whenever a menu with `testId` is asked to open. */
export function useOpenedOnRequest(testId: string, open: () => void): void {
  useEffect(() => {
    const listener = (event: Event) => {
      if ((event as CustomEvent<string>).detail === testId) {
        open();
      }
    };
    window.addEventListener(MENU_OPEN, listener);
    return () => { window.removeEventListener(MENU_OPEN, listener); };
  }, [testId, open]);
}

/** The frames to wait for a form to be drawn: a page changing under a navigation settles well within them. */
const FRAMES_MAX = 60;

/** Focuses the field matching `selector` once it is on screen, opening the folds that hold it. */
export function focusForm(selector: string): void {
  let frames = 0;
  const look = () => {
    const field = document.querySelector<HTMLElement>(selector);
    if (field !== null) {
      for (let fold = field.closest("details"); fold !== null; fold = fold.parentElement?.closest("details") ?? null) {
        fold.open = true;
      }
      field.focus();
      field.scrollIntoView({ block: "nearest" });
    } else if ((frames += 1) < FRAMES_MAX) {
      requestAnimationFrame(look);
    }
  };
  requestAnimationFrame(look);
}
